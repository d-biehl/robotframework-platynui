# Tasks

The mock provider owns its tree and cannot show a release or lost ancestors, so the runtime's behavior is proven with a lazy fake provider, and the providers' behavior with the contract testkit, the live Java tests and the acceptance lanes. The provider rule (group 3) lands before the wrapper fix (group 4), so that no intermediate state loses ancestors.

## 1. Failing tests first — the runtime

- [ ] 1.1 Create `crates/runtime/tests/xdm_release.rs` with a lazy fake provider that uses only the public API (design decision 8):
  - fresh children on every `children()` call;
  - `Weak` parents, with an option to keep the parent strongly;
  - validity switchable per level;
  - counters per tree for created and dropped nodes and for `children()` calls.

  Add the tests:
  - `each_list_is_enumerated_once_per_query`: `children()` calls equal the nodes created, for `//Button`, `count(//*)`, `//Pane/following-sibling::*`, `(//Button)[last()]` and `//Button/preceding::Pane`;
  - `warm_reuse_reads_nothing_new`;
  - `invalid_nodes_are_not_returned`;
  - `owned_ancestors_are_revalidated`: `following-sibling::*` from a held element drops a sibling that turned invalid;
  - `added_node_appears_after_clear`;
  - `uncached_evaluation_releases_the_tree`: `count(//*)`, `/*`, and `evaluate_single` once its result is dropped;
  - `clear_releases_a_cached_snapshot`;
  - `switching_context_frees_the_previous_snapshot`;
  - `dropping_a_stream_early_releases_it`;
  - `failed_compile_after_root_creation_releases_it`, with and without a cache;
  - `revalidation_does_not_grow_a_retained_cache`;
  - `deep_snapshot_releases_without_stack_overflow`: a chain of 10,000 levels whose nodes keep their parents, built on a 64 MiB thread and cleared on a 256 KiB thread;
  - `provider_panic_during_revalidation_does_not_abort`: under `catch_unwind`, a stream partly consumed, `count(/*)`, `clear()`, and then an `is_valid` that panics;
  - `a_held_result_keeps_its_ancestors`: with the strong parent option, `(//Button)[1]` from a cache, then `clear()`; `ancestors()` reaches the window, `count(ancestor::*)` equals its depth, only that chain stays live, and nothing stays live once the node is dropped.

  Verify with `just test-crate platynui-runtime`: the release, revalidation and panic tests fail today; `each_list_is_enumerated_once_per_query`, `warm_reuse_reads_nothing_new`, `invalid_nodes_are_not_returned`, `added_node_appears_after_clear` and `a_held_result_keeps_its_ancestors` pass today and have to keep passing.
- [ ] 1.2 In `crates/runtime/src/runtime/test_fixtures.rs`, add a lazy tree factory whose window exposes an activation that counts, modeled on `RejectingWindowFactory` (`:345-434`). Next to the tests in `runtime/window.rs:247-295`, add:
  - `bring_to_front` for a button from `evaluate_single(None, …)` without a cache;
  - the same after `evaluate_single_runtime_cached` and `clear_cache()`;
  - a button found with a pane as the context, after the slot was replaced;
  - `shutdown()` releases the snapshot before the providers are shut down (the fake records the order).

  Verify that the activation tests pass today (the leak keeps the ancestors) and that the shutdown test fails.

## 2. Failing tests first — the contract testkit

- [ ] 2.1 In `crates/core/src/ui/contract/testkit.rs`, add unit tests for `verify_children_keep_parent(parent, max_children)` and `verify_subtree_released(root, max_nodes)` (design decision 7):
  - a lazy node without the strong parent fails the first;
  - a lazy node with it passes;
  - a tree that owns its children passes the first and is exempt from the second;
  - a node that caches its children fails the second;
  - a parent that the caller still holds makes the first report that it cannot prove the rule.

  Verify with `just test-crate platynui-core` that they fail to compile.

## 3. Providers keep the parent of every listed node

- [ ] 3.1 Implement the two testkit checks. Verify that 2.1 passes.
- [ ] 3.2 Keep the parent at the provider sites of design decision 1:
  - UI Automation: `ElementChildrenIter::next` and `AppWindowIter::next`;
  - JAB: `ChildIter::next` and `JabAppNode::children`;
  - the agent: `AgentNode::children` and `AgentAppNode::children`;
  - AT-SPI: `AtspiNode::children`.

  Reword the keepalive field comments ("normal tree nodes leave this None") and the comment at AT-SPI's `parent_is_application` (`crates/provider-atspi/src/node.rs:59-65`). Verify with `just check` and `just test`.
- [ ] 3.3 A UI Automation unit test that is not ignored, next to `desktop_root_satisfies_the_common_attribute_contract` (`crates/provider-windows-uia/src/node.rs:2132`), runs both checks on the taskbar's element (`Shell_TrayWnd`). Verify on Windows with `just test-crate platynui-provider-windows-uia`, and verify once that it fails with the change of 3.2 reverted for UI Automation.
- [ ] 3.4 Extend the live Java tests (`crates/provider-java/tests/live_fixture.rs`) for the agent and, with the agent disabled, for JAB:
  - reach a table cell through the application node, holding nothing but the check's handles;
  - assert that the cell's ancestors reach the window and the application node, and that `top_level_or_self()` is the window;
  - run both testkit checks on the window.

  Verify on Windows with `just install-provider-java`, then the Java live tests of `just test-acceptance-windows`.
- [ ] 3.5 Write the rule into the documentation of `UiNode::parent` (`crates/core/src/ui/node.rs:30`) and into the provider checklist of `dev-docs/architecture.md` §7.3. Verify by reading.

## 4. Release the snapshots

- [ ] 4.1 In `crates/runtime/src/xpath.rs` (design decisions 2 and 5):
  - the enum carries `Arc<DocumentData>` and `Arc<ElementData>`;
  - `ParentLink` (`Unresolved`, `Linked`, `Owned`) replaces `parent_cache`;
  - `NodeChildrenIter::next` stores `Linked`;
  - `parent()` falls back from a dead `Weak` to a fresh `Owned` parent;
  - `prepare_for_evaluation` on an element follows an `Owned` parent.

  Document the `Owned` invariant at `ParentLink`. Verify that the release tests of 1.1 without deep chains, and `owned_ancestors_are_revalidated`, pass.
- [ ] 4.2 Release without recursion (design decision 3): `Drop` for `DocumentData` and `ElementData` drains iteratively, collects the provider nodes in pre-order and drops them leaves first. Every lock there tolerates poisoning. Verify that `deep_snapshot_releases_without_stack_overflow` and `provider_panic_during_revalidation_does_not_abort` pass, with `just test-crate platynui-runtime` and once with `cargo nextest run -p platynui-runtime --release`.
- [ ] 4.3 Nothing is released while a lock is held (design decision 4):
  - `XdmCache::clear`;
  - the slot replacement in `get_or_create_xdm_root`;
  - `Runtime::clear_cache` through `shared_xpath_cache().clear()`;
  - revalidation (`xpath.rs:556`, `:558`, `:587`, `:589`);
  - the replacement of an `Owned` parent.

  Also drop an exhausted provider iterator (`:1165-1167`). Verify by review and with the full 1.1.
- [ ] 4.4 `Runtime::shutdown` clears the shared cache before the dispatcher and the providers are shut down (design decision 6). Verify that the shutdown test of 1.2 passes.
- [ ] 4.5 Verify that `just check` and `just test` are green, including the runtime tests of `window-activation-state`, the activation tests of 1.2, and all of 1.1.

## 5. Acceptance tests (real providers)

Follow the `robot-test-style` skill.

- [ ] 5.1 In `tests/acceptance/egui/auto_activate.robot`, which has no platform tag and runs on every lane, add:
  - **A Captured Element Still Raises Its Window After The Snapshot Was Discarded:**
    - `Stack Windows`;
    - capture `${button}=    BM.Query    ${BETA}//*[@Id="btn-click-me"]    only_first=${True}`;
    - read its `Bounds`, run another `BM.Query`, then `BM.Activate Window    ${ALPHA}`;
    - `BM.Pointer Click    ${button}`;
    - BETA is active, its click counter went up by one, and the `Bounds` are unchanged.
  - **A Root Inside A Window Still Activates That Window:**
    - `Stack Windows`;
    - `BM.Set Root    ${BETA}//*[@Id="btn-click-me"]/..    scope=LOCAL`, and assert that the root's role is not `Window`;
    - bring ALPHA to the front, then click `.//*[@Id="btn-click-me"]`;
    - BETA is active and its counter went up.

  Verify on the Windows lane that both pass on the build of group 4. Then build once with the wrapper fix but without 3.2, and verify that both fail there. Record that run here.
- [ ] 5.2 The captured-element scenario for Swing under the root pinned to the application node (`tests/acceptance/swing/`, for JAB based on `testapp.resource`, for the agent based on `testapp_agent.resource`), with two instances stacked in the same way. Verify on the Windows lane.

## 6. Documentation

- [ ] 6.1 Documentation:
  - `dev-docs/architecture.md` §9.3: when a snapshot is released (discarded, replaced, at the end of a query without a cache or of a stream, at shutdown); that a held node keeps its own ancestors; that a dead link falls back to reading the current UI and may end a running stream's sibling walk early.
  - `dev-docs/testing-strategy.md` §2.2: runtime lifetime tests use the lazy fake, because the mock owns its tree.
  - `dev-docs/python-bindings.md`: `clear_cache()` frees memory, and a held `UiNode` keeps its ancestors.
  - `dev-docs/planning.md` §3.1 and §10.9: event-driven invalidation is not pursued (maintainer decision), with the reason.

  Verify by reading, and with `grep -n "Option B" dev-docs/planning.md`, which shows it only as not pursued.

## 7. Verification

- [ ] 7.1 Run `just check`, `just test` and `just test-python`, then `just build-native`. Verify that everything is green.
- [ ] 7.2 On Windows, run `just install-provider-java`, then `just test-acceptance-windows`, then `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - everything is green, including 3.4, 5.1 and 5.2;
  - no warning comes from PlatynUI;
  - the process exits with code 0, and the Application event log shows no crash of the Python process at interpreter exit.

  Record the outcome here.
- [ ] 7.3 Measurements on Windows (design decision 8), recorded here:
  - private memory per evaluation for 20 runs with `clear_cache()` before each (`count(/*/*)`, and a search under a large editor window): before this change 52.5 KiB and 14.9 MiB, target about 0;
  - the same search on a retained snapshot, against Notepad and the editor (before this change 0.31 MiB with the editor);
  - the time of `clear_cache()` after the editor snapshot;
  - the latency of the first JAB query after `clear_cache()` on the Swing table, and the total time of the Swing suites before and after;
  - a few minutes of `platynui-cli watch --expression` against a busy application, with flat memory.

  If growth on a retained snapshot remains, or JAB's release latency matters, record a follow-up instead of blocking.
- [ ] 7.4 By hand on Windows:
  - an Inspector search result whose subtree was never expanded still reveals and selects;
  - `platynui-cli pointer click` on a non-window element of a covered window raises that window.

  Record the outcome here.
- [ ] 7.5 On a Linux host, run `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor`, then `robotcode results log --level WARN --execution-messages`. Verify that everything is green, with 5.1 on both lanes, and that no warning comes from PlatynUI. Record the outcome here.

## 8. Commit (only when the user asks)

- [ ] 8.1 Commit in reviewable steps, each lint-clean on its own:
  - the tests and the testkit;
  - the provider rule (e.g. `fix(providers): keep the parent of every listed node alive`);
  - the wrapper fix (e.g. `fix(runtime): release XPath snapshots`);
  - the acceptance tests and documentation.

  Subjects ≤ 72 characters. The runtime commit names in its body that memory is now released, and that a held node keeps its ancestors instead of the whole snapshot.
