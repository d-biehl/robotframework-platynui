# Tasks

The mock provider owns its tree and cannot show a release or lost ancestors, so the runtime's behavior is proven with a lazy fake provider, and the providers' behavior with the contract testkit, the live Java tests and the acceptance lanes. The provider rule (group 3) lands before the wrapper fix (group 4), so that no intermediate state loses ancestors.

## 1. Failing tests first — the runtime

- [x] 1.1 Create `crates/runtime/tests/xdm_release.rs` with a lazy fake provider that uses only the public API (design decision 8):
  - fresh children on every `children()` call;
  - `Weak` parents, with an option to keep the parent strongly;
  - validity switchable per level, and nodes that can be removed and added;
  - counters per tree for created and dropped nodes and for `children()` calls, per node.

  The tests are named after the spec's scenarios; the release tests run with weak and with kept parents:
  - `each_list_is_read_once_per_query`: no node's children are read twice, for `//Button`, `count(//*)`, `//Pane/following-sibling::*`, `(//Button)[last()]` and `//Button/preceding::Pane`;
  - `repeating_a_query_on_a_retained_snapshot_reads_nothing_new`;
  - `a_node_that_is_no_longer_valid_is_not_returned`;
  - `a_sibling_that_has_gone_is_not_returned_from_a_held_element`: `following-sibling::*` from a held element drops a sibling that turned invalid;
  - `an_added_node_appears_only_after_the_snapshot_was_discarded`;
  - `a_query_without_a_retained_snapshot_leaves_nothing_behind`: `count(//*)`, `/*` and `//Button`; and `a_single_result_leaves_nothing_behind_once_dropped`, what `evaluate_single` does;
  - `a_discarded_snapshot_is_released`;
  - `a_snapshot_replaced_by_another_context_is_released`;
  - `a_stream_dropped_early_is_released`;
  - `a_query_that_fails_is_released`: a compile error and an error while the query runs, with and without a retained snapshot;
  - `revalidation_does_not_grow_a_retained_snapshot`;
  - `a_deep_snapshot_is_released_without_overflowing_the_stack`: a chain of 10,000 levels whose nodes keep their parents, built on a 64 MiB thread and cleared on a 256 KiB thread;
  - `a_provider_that_panics_does_not_abort_the_process`: under `catch_unwind`, a stream partly consumed, `count(/*)`, and then an `is_valid` that panics; afterwards the stream is dropped and the snapshot cleared;
  - `a_held_result_keeps_its_ancestors`: with kept parents, the first button below the first pane from a cache, then `clear()`; `ancestors()` reaches the window, `count(ancestor::*)` counts the pane and the window, only that chain stays live, and nothing stays live once the node is dropped.

  Added after the review of group 4, each confirmed once to fail on a mutation of the fix:
  - `a_listing_that_panics_does_not_abort_the_process`, with and without a retained snapshot: a list that panics while it is read poisons its lock; the snapshot still answers `count(//*)` afterwards and is released (fails with locks that do not tolerate poison);
  - `a_deep_snapshot_of_a_held_element_is_released_without_overflowing_the_stack`: `count(ancestor::*)` from the deepest node of a 10,000-level chain, then only the snapshot holds it, cleared on a 256 KiB thread (overflows while the element keeps its provider node until after the release);
  - `concurrent_queries_on_one_snapshot_see_every_node`: two threads query one retained snapshot 50 times (fails without the re-checks of design decision 10).

  Verify with `just test-crate platynui-runtime` on the unchanged code. Outcome (2026-09-27): 10 of 36 cases pass — `each_list_is_read_once_per_query` (5 cases), `repeating_a_query_on_a_retained_snapshot_reads_nothing_new`, `a_node_that_is_no_longer_valid_is_not_returned`, `an_added_node_appears_only_after_the_snapshot_was_discarded` and the compile error without a snapshot (2 cases), which have to keep passing. The other 26 fail: every release test, `a_sibling_that_has_gone_is_not_returned_from_a_held_element`, `a_provider_that_panics_does_not_abort_the_process` (its snapshot is never released), and `a_held_result_keeps_its_ancestors`, whose ancestors survive today but only because the whole snapshot leaks.
- [x] 1.2 In `crates/runtime/src/runtime/test_fixtures.rs`, add a lazy tree factory whose window exposes an activation that counts, modeled on `RejectingWindowFactory` (`:345-434`). Next to the tests in `runtime/window.rs:247-295`, add:
  - `bring_to_front` for a button from `evaluate_single(None, …)` without a cache;
  - the same after `evaluate_single_runtime_cached` and `clear_cache()`;
  - a button found with a pane as the context, after the slot was replaced;
  - `shutdown()` releases the snapshot before the providers are shut down (the fake records the order).

  - `bring_to_front_needs_the_provider_to_keep_the_ancestors`: with a lazy tree whose nodes keep their parent only weakly, `bring_to_front` after `clear_cache()` reports `PatternMissing`, which shows that the activation tests rely on the provider rule, not on the snapshot (added after the review).

  Verify that the activation tests pass today (the leak keeps the ancestors) and that the shutdown test fails. Outcome (2026-09-27): `bring_to_front_activates_the_window_of_a_result_without_a_snapshot`, `…_after_the_snapshot_was_discarded` and `…_of_a_node_found_below_a_pane` pass; `shutdown_releases_the_snapshot_before_the_providers` fails.

## 2. Failing tests first — the contract testkit

- [x] 2.1 In `crates/core/src/ui/contract/testkit.rs`, add unit tests for `verify_children_keep_parent(parent, max_children)` and `verify_subtree_released(root, max_nodes)` (design decision 7):
  - a lazy node without the strong parent fails the first;
  - a lazy node with it passes;
  - a tree that owns its children keeps its parents reachable, so no child is reported, but the first check cannot attribute that to the children and reports that it cannot prove the rule; the tree fails the second and is exempt from it;
  - a node that caches its children fails the second;
  - children of which only the first keeps the parent report the others (added after the review: a single child that keeps the parent made every sibling's parent reachable, so the first version of the check passed);
  - a parent that the caller still holds makes the first report that it cannot prove the rule.

  Verify with `just test-crate platynui-core` that they fail to compile.

## 3. Providers keep the parent of every listed node

- [x] 3.1 Implement the two testkit checks. Verify that 2.1 passes.
- [x] 3.2 Keep the parent at the provider sites of design decision 1:
  - UI Automation: `ElementChildrenIter::next` and `AppWindowIter::next`;
  - JAB: `ChildIter::next` and `JabAppNode::children`;
  - the agent: `AgentNode::children` and `AgentAppNode::children`;
  - AT-SPI: `AtspiNode::children`.

  Reword the keepalive field comments ("normal tree nodes leave this None") and the comment at AT-SPI's `parent_is_application` (`crates/provider-atspi/src/node.rs:59-65`). Verify with `just check` and `just test`. Outcome (2026-09-27): clippy is clean for the Windows providers and, for the Linux target, for AT-SPI; the unit tests of core, runtime and the Windows providers pass, except `shutdown_releases_the_snapshot_before_the_providers`, which waits for group 4. The full `just check` and `just test` run in 4.5.
- [x] 3.3 A UI Automation unit test that is not ignored, next to `desktop_root_satisfies_the_common_attribute_contract` (`crates/provider-windows-uia/src/node.rs:2132`), runs both checks on a window with three standard buttons that a child process of the test shows off screen, and the first check on that process's application node, whose windows come from `AppWindowIter`. The test depends on no window or application that Windows brings, since those change between versions. Verify on Windows with `just test-crate platynui-provider-windows-uia`, and verify once that it fails with the change of 3.2 reverted for UI Automation. Outcome (2026-09-27): `listed_nodes_keep_their_parent_and_nothing_holds_its_children` passes; with the two `hold_parent` calls of UI Automation removed it fails with `ChildParentUnreachable` for the window's buttons, and with only the one in `AppWindowIter` removed it fails for the application's window. (The first version used the taskbar; it was replaced on 2026-09-27, because the taskbar changes between Windows versions.)
- [x] 3.4 Extend the live Java tests (`crates/provider-java/tests/live_fixture.rs`) for the agent and, with the agent disabled, for JAB:
  - reach a table cell through the application node, holding nothing but the check's handles;
  - assert that the cell's ancestors reach the window and the application node, and that `top_level_or_self()` is the window;
  - run both testkit checks on the window.

  Verify on Windows with `just install-provider-java`, then the Java live tests of `just test-acceptance-windows`. Outcome (2026-09-27): `live_jab_listed_nodes_keep_their_ancestors` and `live_agent_listed_nodes_keep_their_ancestors` pass in both lane runs; the provider-java live tests 30 of 30.
- [x] 3.5 Write the rule into the documentation of `UiNode::parent` (`crates/core/src/ui/node.rs:30`) and into the provider checklist of `dev-docs/architecture.md` §7.3. Verify by reading.

## 4. Release the snapshots

- [x] 4.1 In `crates/runtime/src/xpath.rs` (design decisions 2 and 5):
  - the enum carries `Arc<DocumentData>` and `Arc<ElementData>`;
  - `ParentLink` (`Unresolved`, `Linked`, `Owned`) replaces `parent_cache`;
  - `NodeChildrenIter::next` stores `Linked`;
  - `parent()` falls back from a dead `Weak` to a fresh `Owned` parent;
  - `prepare_for_evaluation` on an element follows an `Owned` parent.

  Document the `Owned` invariant at `ParentLink`. Verify that the release tests of 1.1 without deep chains, and `a_sibling_that_has_gone_is_not_returned_from_a_held_element`, pass. Outcome (2026-09-27): all 36 cases of 1.1 pass. Beyond the design, attribute wrappers are behind an `Arc` too (clippy's `large_enum_variant`), and the content a document and an element share moved into one `LazyContent`.
- [x] 4.2 Release without recursion (design decision 3): `Drop` for `DocumentData` and `ElementData` drains iteratively, collects the provider nodes in pre-order and drops them leaves first. Every lock there tolerates poisoning. Verify that `a_deep_snapshot_is_released_without_overflowing_the_stack` and `a_provider_that_panics_does_not_abort_the_process` pass, with `just test-crate platynui-runtime` and once with `cargo nextest run -p platynui-runtime --release`. Outcome (2026-09-27): both pass, in debug and in release. The design's reverse pre-order would still release an owned chain of ancestors recursively, because an owned parent is visited after its element; `Release::run` therefore drops the provider nodes by their depth relative to where the release started (children one level down, owned parents one level up), deepest first.
- [x] 4.3 Nothing is released while a lock is held (design decision 4):
  - `XdmCache::clear`;
  - the slot replacement in `get_or_create_xdm_root`;
  - `Runtime::clear_cache` through `shared_xpath_cache().clear()`;
  - revalidation (`xpath.rs:556`, `:558`, `:587`, `:589`);
  - the replacement of an `Owned` parent.

  Also drop an exhausted provider iterator (`:1165-1167`). Verify by review and with the full 1.1. Outcome (2026-09-27): reviewed, also by the adversarial review; beyond the list above, providers' `is_valid` and `children()` are no longer called while a lock of the snapshot is held. All of 1.1 passes.
- [x] 4.4 `Runtime::shutdown` clears the shared cache before the dispatcher and the providers are shut down (design decision 6). Verify that the shutdown test of 1.2 passes.
- [x] 4.5 Verify that `just check` and `just test` are green, including the runtime tests of `window-activation-state`, the activation tests of 1.2, and all of 1.1. Outcome (2026-09-27): both green, `just test` with 2,378 tests.
- [x] 4.6 UI Automation frees the array `GetRuntimeId` returns, on every path (design decision 9). Verify with the memory measurement of 7.3: discarding a snapshot of a large window no longer grows memory linearly. Outcome (2026-09-27): before the fix, 80 discarded snapshots of a VS Code window grew private memory by 47 MiB, linearly (0.6 MiB each); with it, memory stays within 0.3 to 0.6 MiB of the start. A walk through `children()` alone grew by 0.53 MiB per walk only when it read each node's runtime id, which located the leak.

## 5. Acceptance tests (real providers)

Follow the `robot-test-style` skill.

- [x] 5.1 In `tests/acceptance/egui/auto_activate.robot`, which has no platform tag and runs on every lane, add:
  - **A Captured Element Still Raises Its Window After The Snapshot Was Discarded:**
    - `Stack Windows`;
    - capture `${button}=    BM.Query    ${BETA}//*[@Id="btn-click-me"]    only_first=${True}`;
    - read its `Bounds`, run another `BM.Query`, then `BM.Activate Window    ${ALPHA}`;
    - `BM.Pointer Click    ${button}`;
    - BETA is active, its click counter went up by one, and the `Bounds` are unchanged.
  - **A Root Inside A Window Still Activates That Window** (needs a container the accessibility tree keeps: the egui test app exposes its button row as a group with the id `row-buttons`, because AccessKit drops egui's plain containers):
    - `Stack Windows`;
    - `BM.Set Root    ${BETA}//*[@Id="btn-click-me"]/..    scope=LOCAL`, and assert that the root's role is not `Window`;
    - bring ALPHA to the front, then click `.//*[@Id="btn-click-me"]`;
    - BETA is active and its counter went up.

  Verify on the Windows lane that both pass on the build of group 4. Then build once with the wrapper fix but without 3.2, and verify that both fail there. Record that run here.

  Outcome (2026-09-27, Windows lane):
  - Both pass with the rule.
  - On a build with the wrapper fix but without 3.2, the root scenario fails. From the pinned root, even the absolute `${ALPHA}/@IsActive` finds nothing, because `/` no longer reaches the desktop.
  - The captured-element scenario passed there at first. UI Automation hands out `Activatable` for every element, not only for windows, and its `activate()` focuses the element, which raises the window without any ancestors. The test now also checks that the captured button still reaches its window (`ancestor::*[self::Window or self::Frame]` with the button as root), and that check fails without the rule.
  - The first lane run found an ordering dependency: egui applies a press at the pointer position it last saw over its window, so a click on a window raised under a resting pointer is lost. `Move The Pointer Off The Button` now rests the pointer on the covering window before each click through a raise. It was added to the existing `Auto Activate Raises The Background Window For A Pointer Click` too, which failed the same way in three runs of the suite on its own.
- [x] 5.2 The captured-element scenario for Swing under the root pinned to the application node (`tests/acceptance/swing/`, for JAB based on `testapp.resource`, for the agent based on `testapp_agent.resource`), with two instances stacked in the same way. Verify on the Windows lane. Outcome (2026-09-27): `tests/acceptance/swing/auto_activate.robot` (JAB) and `agent_auto_activate.robot` (agent) pass with the rule, and both fail without it: the captured button's window is not raised.

## 6. Documentation

- [x] 6.1 Documentation:
  - `dev-docs/architecture.md` §9.3: when a snapshot is released (discarded, replaced, at the end of a query without a cache or of a stream, at shutdown); that a held node keeps its own ancestors; that a dead link falls back to reading the current UI and may end a running stream's sibling walk early.
  - `dev-docs/testing-strategy.md` §2.2: runtime lifetime tests use the lazy fake, because the mock owns its tree.
  - `dev-docs/python-bindings.md`: `clear_cache()` frees memory, and a held `UiNode` keeps its ancestors.
  - `dev-docs/planning.md` §3.1 and §10.9: event-driven invalidation is not pursued (maintainer decision), with the reason.

  Verify by reading, and with `grep -n "Option B" dev-docs/planning.md`, which shows it only as not pursued. Outcome (2026-09-27): done; the grep also finds an unrelated "Option B" of the builder design at `planning.md:327`.

## 7. Verification

- [x] 7.1 Run `just check`, `just test` and `just test-python`, then `just build-native`. Verify that everything is green. Outcome (2026-09-27): green; `just test` 2,378 tests, `just test-python` 876 tests.
- [x] 7.2 On Windows, run `just install-provider-java`, then `just test-acceptance-windows`, then `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - everything is green, including 3.4, 5.1 and 5.2;
  - no warning comes from PlatynUI;
  - the process exits with code 0, and the Application event log shows no crash of the Python process at interpreter exit.

  Record the outcome here.

  Outcome (2026-09-27):
  - First run: 118 of 119. The failure was the ordering dependency of 5.1.
  - Final run: 118 of 119. The failure was `Activating A Context Submenu Item Updates The Last Action` (QML), a submenu that opens on hover; it passed in the first run and in three reruns of its suite.
  - No WARN or ERROR message from PlatynUI, and no crash in the Application event log.
  - The live tests of `platynui-java-agent`, which this change does not touch, failed in two of four further runs with `NotAJvm` when attaching to a freshly started JVM, each time in a different test; they passed in both lane runs.
- [x] 7.3 Memory measurements (design decision 8), recorded here. They run only against an application the repository owns, and every query is scoped to that application's `app:Application` node.
  - [x] 7.3.1 Add `apps/large-tree-qt/main.py`, a PySide6 window with a large, deterministic tree.
    - It shows `--groups` group boxes (`group-<g>`) of `--items` widgets each (`item-<g>-<i>`), cycling through a push button, a label and a check box, under stable accessible names.
    - It is a measurement helper, not a fixture of the blueprint. `README.md` in its directory says so and describes how to run the measurement.
    - Exclude the directory from the Cargo workspace, check both scripts with mypy in `just mypy`, and add one line to `dev-docs/testing-strategy.md` §5.

    Verify: `just check` is clean, and the app starts and shows the expected number of widgets.
  - [x] 7.3.2 Add `apps/large-tree-qt/measure_snapshot_memory.py`.
    - It starts the app and waits until the app's widgets are on the tree, found through the application node of the launched process.
    - It evaluates `.//*[@Name='x-not-there']` under that node many times, once keeping the snapshot and once discarding it before each run.
    - It reports the growth of its own private memory per evaluation and per element, over several rounds so the noise shows, and the time of `clear_cache()` after a full snapshot.
    - Windows reads `PrivateUsage` through `GetProcessMemoryInfo`; Linux reads anonymous and swapped memory from `/proc/self/smaps_rollup`.
    - It is read-only, with no pointer or keyboard input, and it ends the app when it is done.

    Verify: `just check` is clean, and a run on Windows completes and ends the app.

    Outcome (2026-09-27): `just check` is clean. mypy also passes with `--platform linux` for the `/proc` branch. Every run ended the app.
  - [x] 7.3.3 Run it on Windows against a release build of this change, and against a release build of `0802be7`, which is before this change, as the positive control. Verify:
    - with this change, the growth per discarded snapshot stays within the noise of the retained runs;
    - the positive control grows clearly on the same tree.

    Record both here.

    Outcome (2026-09-27, Windows 11, release builds, 50 groups of 20 widgets, 1,063 elements under the application node). This change was built from `fce970c`, whose Rust code carries it. `0802be7` was built in a git worktree with its own environment.

    | Build | Snapshot | Runs per round | Growth per evaluation, 3 rounds | Per element | Time per evaluation |
    |---|---|---|---|---|---|
    | this change | retained | 100 | 2.0, 0.0, 0.0 KiB | at most 1.9 B | 105 ms |
    | this change | discarded | 100 | −0.4, 5.7, −1.0 KiB | at most 5.5 B | 349 ms |
    | `0802be7` | retained | 30 | 0.0, 0.0, 0.0 KiB | 0 B | 112 to 117 ms |
    | `0802be7` | discarded | 30 | 4,236.8, 4,253.9, 4,263.9 KiB | about 4,100 B | 353 to 361 ms |

    `clear_cache()` after a full snapshot takes:
    - 3.4 to 5.5 ms with this change;
    - 0.0 ms before it, where nothing was released.

    The results:
    - The positive control grows by about 4.1 MiB per discarded snapshot, about 4,100 bytes per element. That matches the first measurement under the editor. The positive control used fewer runs per round only to keep the leaking process small.
    - With this change the growth stays within ±6 KiB per evaluation, below 6 bytes per element. That is at least 700 times less than the leak.
    - The measurement can also see a leak of the size of 4.6. At about 100 bytes per element (0.63 MiB for about 6,000 elements), that leak would show as about 110 KiB per evaluation here.
  - [x] 7.3.4 Run the same pair on a Linux host, inside the lanes' session scripts, which bring up AT-SPI and enable Qt's accessibility. For example:

    ```sh
    scripts/startxsession.sh -- scripts/platynui-robot-session.sh uv run python apps/large-tree-qt/measure_snapshot_memory.py
    ```

    Record it here.

    Outcome (2026-09-28, CachyOS, X11 session on Xvfb through `startxsession.sh` and `platynui-robot-session.sh`, AT-SPI, Python 3.12, PySide6 6.11, release builds, 50 groups of 20 widgets, 1,057 elements under the application node):
    - This change was built from `4b1fc6c`, which carries it.
    - `0802be7` was built in a git worktree with its own environment.
    - Each run used its environment's `.venv/bin/python` directly rather than `uv run`, so that a sync could not replace the native build.

    | Build | Snapshot | Runs per round | Growth per evaluation, 3 rounds | Per element | Time per evaluation |
    |---|---|---|---|---|---|
    | this change | retained | 100 | 0.4, 0.0, 0.0 KiB | at most 0.4 B | 101 to 102 ms |
    | this change | discarded | 100 | 0.2, 0.1, 0.0 KiB | at most 0.2 B | 212 to 215 ms |
    | `0802be7` | retained | 30 | 0.0, 0.0, 0.0 KiB | 0 B | 106 to 112 ms |
    | `0802be7` | discarded | 30 | 2,019.5, 2,019.9, 2,019.7 KiB | about 1,960 B | 219 to 222 ms |

    `clear_cache()` after a full snapshot takes:
    - 0.6 to 1.0 ms with this change;
    - 0.0 ms before it, where nothing was released.

    The results:
    - The positive control grows by about 2.0 MiB per discarded snapshot, about 1,960 bytes per element, the same in every round. That is about half of the leak per element under UI Automation on Windows; the providers' nodes differ in size.
    - With this change the growth stays below 0.5 KiB per evaluation and below 0.5 bytes per element. That is more than 8,000 times less than the leak.
    - Both runs ended the app and tore the session down.

  Dropped from the earlier plan, each for its reason:
  - **The first JAB query's latency after `clear_cache()`, and the Swing suite times.** Only JAB releases native objects one by one (`crates/provider-java-jab/src/handle.rs:39-44`). The agent backend, the preferred path for Java, holds no host-side resource per node. The cost concerns only the JAB fallback (design, Open Questions).
  - **`platynui-cli watch --expression` against a busy application.** No real provider emits events; only the mock declares event capabilities. So `watch` waits without output and never re-evaluates. The risk behind it, a retained snapshot growing through revalidation, is covered by `revalidation_does_not_grow_a_retained_snapshot` in `crates/runtime/tests/xdm_release.rs`.
  - **The search under a large editor window.** An editor is not part of the repository, and its tree changes between runs.

  First, exploratory measurement (2026-09-27, Windows 11, release build of the extension, UI Automation only with the Java provider disabled, scratch scripts that are not in the repository):

  | Query | Snapshot | Before this change | With it |
  |---|---|---|---|
  | `count(/*/*)`, 300 times | discarded before each | 52.5 KiB per evaluation | 0.6 KiB (noise) |
  | `count(/*/*)`, 300 times | retained | 0 | 0 |
  | `.//*[@Name='x-not-there']` under a VS Code window | discarded before each | 14.9 MiB (3 runs, window of about 3,500 elements) | 0.01 MiB (20 runs, 5,300 to 6,000 elements) |
  | the same | retained | 0.31 MiB (3 runs) | 0.02 MiB (20 runs) |

  `clear_cache()` after the VS Code snapshot takes 31 to 44 ms. The first measurement with the wrapper fix still grew by 0.63 MiB per discarded snapshot, linearly; that was the UI Automation leak of 4.6.

  This measurement is not repeatable, and it is kept only as the first evidence and as how the leak of 4.6 was found:
  - `count(/*/*)` covers the whole desktop;
  - the editor's tree changes between runs;
  - the two columns of the editor search differ in runs and tree size.
- [x] 7.4 By hand on Windows:
  - an Inspector search result whose subtree was never expanded still reveals and selects;
  - `platynui-cli pointer click` on a non-window element of a covered window raises that window.

  Record the outcome here. Outcome (2026-09-27): the maintainer confirmed that the Inspector and the CLI work locally with this change.
- [x] 7.5 On a Linux host, run `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor`, then `robotcode results log --level WARN --execution-messages`. Verify that everything is green, with 5.1 on both lanes, and that no warning comes from PlatynUI. Record the outcome here. Outcome (2026-09-27): run by CI on push (run 36335069131, commit `cbf8e1b`): `Acceptance (Linux, x11)` 83 of 83 and `Acceptance (Linux, compositor)` 84 of 84, with no WARN or ERROR in either log. The first push had failed ten egui tests on the compositor lane: the button group of the egui test app had no bounds, so AT-SPI placed its buttons at window-local coordinates; `cbf8e1b` gives the group its bounds.

## 8. Commit (only when the user asks)

- [x] 8.1 Commit in reviewable steps, each lint-clean on its own:
  - the tests and the testkit;
  - the provider rule (e.g. `fix(providers): keep the parent of every listed node alive`);
  - the wrapper fix (e.g. `fix(runtime): release XPath snapshots`);
  - the acceptance tests and documentation.

  Subjects ≤ 72 characters. The runtime commit names in its body that memory is now released, and that a held node keeps its ancestors instead of the whole snapshot.

  Outcome (2026-09-27), each commit checked with `cargo fmt --check` and clippy on exactly its staged state:
  - `33fa58c test: add snapshot release tests and testkit ownership checks`
  - `21f9429 fix(providers): keep the parent of every listed node alive`
  - `f5569db fix(uia): free the runtime-id array of every element`
  - `f171368 fix(runtime): release XPath snapshots`
  - `d7cd473 test(acceptance): prove held elements and roots keep their window`
  - `01517c6 docs: describe snapshot lifetimes and the provider parent rule`
  - `docs(openspec): record the implementation of xdm-snapshot-release`
