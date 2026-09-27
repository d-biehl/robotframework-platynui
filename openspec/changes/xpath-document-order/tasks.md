# Tasks

Rules for every task:

- No test or measurement uses the taskbar, the shell or applications that Windows ships.
- Queries used for measurements are scoped to a repository test app, for example `count(.//*)` with the root `/app:Application[@ProcessId=${pid}]`. They are never desktop-wide.
- Running the acceptance lane takes over the pointer and the keyboard. Ask the maintainer before each run.

## 1. Before the change

- [ ] 1.1 Record the baseline on Windows, with the maintainer's go-ahead:
  1. `just install-provider-java`.
  2. `just test-acceptance-windows --profile real-windows run --suite "Agent Table" --suite "Native Attributes"`.
  3. From `results/output.xml` (`uv run --no-sync robotcode results`), note the keyword times of the tests that address cells by position (`agent_table.robot:50-129`, `native_attributes.robot:49-68`).
  4. The time of `Query    count(.//*)    only_first=${True}` with the root set to the fixture's `/app:Application[@ProcessId=${pid}]`.

  Record the numbers here.
- [ ] 1.2 Check whether `xdm-snapshot-release` has been archived. If it has not, groups 4 and 8 leave its artifacts alone and keep the existing tests of `crates/runtime/tests/xdm_release.rs` unchanged. Record the state here.

## 2. Tests first — Robot Framework on the mock

Follow the `robot-test-style` skill.

- [ ] 2.1 Add `tests/BareMetal/document_order.robot` with the six scenarios of `baremetal-selector-resolution` ("Several matches resolve to the first in document order").
  - Import: `Library    PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}`, with the root set to `//control:Window[@Name="Operations Console"]`, in the style of `selector_resolution.robot`.
  - The suite documentation explains `(.//X)[n]` against `.//X[n]`.

  Verify that all six fail today: `just test-baremetal --suite "Document Order"`.

## 3. Tests first — XPath engine

- [ ] 3.1 Fixtures in `crates/xpath/tests/it/common/mod.rs`:
  - a keyless wrapper around `SimpleNode` (no `doc_order_key`, `compare_document_order` by ancestry);
  - a keyless wrapper that records whose `children()` were read;
  - builders for the spec trees `r:[A:[X1],X2]`, `r:[X1:[X2:[Y1],Y2]]`, `r:[a1:[a2,c1]]`, `W:[P1:[B1,B2],B3,P2:[B4,B5]]`, `r:[a,b,c]`, `r:[a:[b1,b2],c]`, `r:[x1,x2,y1,x3,y2]`, `r:[X1:[B1,X2:[B2]]]`;
  - the wide tree `W` → 50 `P` → 20 `B`;
  - a keyless tree with one node missing from its parent's list;
  - two separate documents.

  Declare every new test module in `tests/it/main.rs` (`:1-6`). Verify that `just test-crate platynui-xpath` builds.
- [ ] 3.2 Add `document_order.rs`. It is a table over the scenarios of "A path returns its nodes in document order without duplicates", and each row runs on the keyed and on the keyless model. Include the missing-node and two-roots scenarios; the atomic scenario already exists at `evaluator_path_filter_expr.rs:40-53`. Verify that the rows marked "before this change" fail today, and that the `(//B)[last()]` and `//B/..` controls pass.
- [ ] 3.3 Add `positional_predicates_per_step.rs` with the scenarios of "A positional predicate in a step counts per context node" and "A predicate on a parenthesized expression counts over the whole sequence", keyed and keyless. Verify that the rows with a "before this change" note fail today, `//B[3]` included. Also verify that `//B[0]`, `//B[1.5]`, `(//B)[6]`, the atomic-sequence row and `//B[(1, 2)]` → `FORG0006` pass.
- [ ] 3.4 Add `document_order_oracle.rs`:
  - It enumerates every ordered tree of up to six element nodes labelled `a` or `b`.
  - It compares, on the keyed and keyless models, `//a`, `//a[1]`, `//a[last()]`, `(//a)[2]`, `//a/b`, `//a/b[1]`, `//*/following-sibling::a`, `//a/following::b`, `//a/following::b[1]`, `//a/preceding::b[1]`, `//a/ancestor::*[1]`, `//a/descendant::b[1]` and `//*/..` against a naive evaluator written in the test itself. The naive evaluator lists each axis from each context by pre-order index, applies predicates per context, then sorts and deduplicates, and shares no code with the engine.

  Verify that it fails today and names the first failing tree and expression.
- [ ] 3.5 Add `first_match_reads.rs`, using the recording wrapper on the wide tree. For the first item of `//B[@id='B0_3']`, `//B`, `(//B)[1]`, `//B[1]`, `//P/B[1]`, `//P/B[@id='B0_3']`, `//W/P[@id='P0']` and `.//B[2]` from `W`, assert three things:
  - the item equals the first item of the full result;
  - every list read belongs to the item, one of its ancestors, or a node before it;
  - the count stays within the design's bounds: 6, 3, 3, 3, 3, 6, 2 and 3.

  Also assert that the first item of `//B/..` on `W:[P1:[B1,B2],B3,P2:[B4,B5]]` is `W`, and that a full `//B` reads 1,052 lists. Verify that the order assertions fail today where the order is wrong (for example `//B` gives `B3` first on the small tree).
- [ ] 3.6 Add `compiler_document_order_plan.rs`. It asserts:
  - no normalization op for `//B[@id='x']`, `Window[@Name='x']//Button[@Name='y']`, `//Window/Button[@Name='OK']`, `.//B[2]`, `(//B)[1]` and `.//*[@Name='t']/*[3]/*[2]`;
  - one normalization op for `//B/..`, `//B/ancestor::*`, `(//c, //a)/self::*` and `//*/following-sibling::*`;
  - pushdown of the leading non-positional run only: `(//T)[@a][1]` moves `[@a]` and keeps `[1]`, and `(//T)[1][@a]` moves nothing;
  - `[1]`, `[$n]`, `[count(x)]` and `[position() < 3]` never move into a step;
  - `[@a]`, `[@a='x']` and `[contains(@a,'x')]` move.

  It names the opcodes of design decisions 1 and 4. Verify that it does not compile yet.
- [ ] 3.7 Update the tests that assert the old behavior:
  - In `evaluator_more.rs:158-181`, the helper `:41-60` counts position per section, so the count becomes 48 instead of 53.
  - The shape tests of `compiler_paths_predicates.rs` (`:8-14`, `:22-36`, `:38-49`, `:63-68`, `:164-194`, `:196-222`, `:259-272`) move to the new plan and predicate type.
  - The unit tests of `crates/xpath/src/compiler/optimizer.rs:312-396`: positional literals no longer merge.
  - The comments in `evaluate_first.rs:98-99` and `optimizer_integration.rs:51-61`.

  Verify that each of them fails or does not compile until group 6 lands.

## 4. Tests first — runtime, Python and PlatynUI.core

- [ ] 4.1 In `crates/runtime/tests/xdm_release.rs`, next to its lazy fake:
  - the first item of the evaluation stream for `//Button` is `root/0/0/0`, and the lists read are only those of `root`, `root/0` and `root/0/0`;
  - `(//Button)[2]` is `root/0/0/0/0`;
  - extend `each_list_is_read_once_per_query` with a union that mixes attributes the fake serves and elements, so the sort has to place attributes.

  With the mock (`crates/runtime/src/runtime/evaluation.rs` tests), the names of `(//control:Window[@Name='Operations Console'])[1]//item:TreeItem` come out in document order. Verify with `just test-crate platynui-runtime` that the order and read assertions fail today and the drop-count tests stay green.
- [ ] 4.2 In `packages/native/tests/test_mock_tree_content.py`, add:
  - `evaluate('(//control:Window[@Name="Operations Console"])[1]//item:TreeItem')` gives the names in document order;
  - the same path with `[2]` on the last step gives Metrics, Reports and Monatlich;
  - `evaluate_single` on the same path with `[@Name!="Dashboard"]` gives Overview.

  They fail until the mock rebuild in 10.2.
- [ ] 4.3 In `tests/PlatynUI/test_locator.py`, add the rendering scenarios of `core-locator` and change `test_index_and_position` (`:117-119`) to `descendant::Button[position()=3][1]`. In `tests/PlatynUI/test_adapter_factory.py`, next to the mock-backed `RuntimeAdapterFactory` tests, add `find_one` with `index=2` (Overview) and `index=8` (None), and `find_all` in document order. Verify with `uv run pytest tests/PlatynUI/test_locator.py` that the rendering tests fail today.

## 5. Tests first — Java agent ids per view

- [ ] 5.1 In `crates/provider-java/tests/live_fixture.rs`:
  - Add an ignored live test for the scenarios of "Agent runtime ids are scoped per view": two nodes and two id forms for the fixture window, the prefixes of one button in both views, and `SelectedItems` per view.
  - Change the hit-test check of `live_agent_serves_table_cells_the_bridge_cannot` (`:1146-1155`). It compares the picked cell with a cell of the flat window. It must compare with the same cell reached through the fixture's `app:Application` node, and check that the pick's ancestors lead to that node.
  - Keep `live_two_hosts_share_one_agent_and_agree_on_identity` unchanged.

  Verify with `cargo check -p platynui-provider-java --tests`. The tests run in the Windows lane (12.2).
- [ ] 5.2 In `tests/acceptance/swing/agent_table.robot`, add "The Window Is One Element Per View". It evaluates the union of the flat window and the window under its `app:Application` node, both scoped to the fixture's title and `@Technology="JavaAgent"`, and asserts two elements with different `RuntimeId`s. It fails today with one element; verify in 12.2.

## 6. Engine — predicates

- [ ] 6.1 Classify each predicate when it is lowered (design decision 1). Carry the flag in the IR, and print it in `Display` and `fmt_with_indent`. Add unit tests for the classifier: the non-positional and possibly-positional cases of the decision, `position()` inside a nested step and inside `for`, `some` and `every`. Verify with `just test-crate platynui-xpath` that the classifier tests pass and the crate builds.
- [ ] 6.2 Evaluate a step's predicates per context node when one of them may be positional, and allow the three minimizing cursors only when none is (design decision 2). Verify that the per-context rows of 3.3 whose result does not depend on order now pass, for example `count(//B[1])` and `//B[3]`.
- [ ] 6.3 Push down only the leading non-positional predicates, and rewrite a predicate-free `descendant-or-self::node()` followed by a non-positional `child::T[…]` into `descendant::T[…]` after pushdown (design decision 3). Correct the doc comments at `optimizer.rs:11-51` and `:129-158`. Verify that the pushdown assertions of 3.6 and the updated unit tests of 3.7 pass.

## 7. Engine — order

- [ ] 7.1 Add the optional identity hint to `XdmNode` (default `None`), implement it for `SimpleNode` and the test wrappers, and let `DistinctCursor` use it with the linear fallback kept (design decision 8). Add a unit test that deduplicates hinted and unhinted nodes. Verify with `just test-crate platynui-xpath`.
- [ ] 7.2 Add the normalization cursor with the total order of design decision 6:
  - by keys when all nodes are keyed;
  - otherwise by sibling-index paths built once per parent list through the hint;
  - attributes after their owner by (namespace URI, local name);
  - a deterministic slot for a node missing from its parent's list;
  - the order in which roots first appeared for different roots;
  - atomics unchanged;
  - the cancellation flag checked while draining.

  Replace `EnsureOrderCursor`, and use the same order in the set operations. Add unit tests for each case, and one where the sort sees no call to `attributes()`. Verify that they pass and that `evaluator_path_filter_expr.rs` stays green.
- [ ] 7.3 Track single, ordered, distinct and peer through a path, and emit normalization according to the table of design decision 4, including `(E)/…` bases, filter-expression steps and `(E)[p]`. Verify that 3.2 and the normalization assertions of 3.6 pass.
- [ ] 7.4 Minimize `following::` by the earliest subtree end (design decision 7), with a unit test on `r:[a1:[a2,c1]]`. Verify that `//a/following::c` in 3.2 passes.
- [ ] 7.5 Add the ordered child merge and the bounded advance (design decision 5), and answer the bounded advance in the descendant walkers and in the merge itself. Verify that 3.3, 3.4 and 3.5 pass, and that the no-normalization assertions of 3.6 pass.
- [ ] 7.6 Correct the remaining engine docs: `evaluator/mod.rs:220-244` (`(//item)[1]` as the first-item example), `ir.rs:99-104` and the comment at `cursors.rs:529`. Verify that `just test-crate platynui-xpath` is fully green, the tests updated in 3.7 included, and that `just clippy` is clean.

## 8. Runtime

- [ ] 8.1 Implement the identity hint for `RuntimeXdmNode` from what its equality compares. Give `AttributeData` a weak link to the element wrapper that listed it, and let `parent()` of an attribute use it while it lives (design decisions 6 and 8). Update the cycle note at `crates/runtime/src/xpath.rs:414-425`. Verify with `just test-crate platynui-runtime` that 4.1 passes and that every existing test of `xdm_release.rs` stays green, the drop counts and the read-once rule included.

## 9. Java agent ids per view

- [ ] 9.1 Give `AgentNode` its view, fixed where it is created and passed to every child:
  - flat in the backend's sweep (`crates/provider-java/src/agent/backend.rs:384-404`);
  - the application view in `AgentAppNode::children` (`agent/app.rs:125-137`) and in `build_chain` (`backend.rs:498-514`).

  Build the runtime id as `agent/<pid>/<id>` or `agent/app/<pid>/<id>` (`agent/node.rs:224`), and the `SelectedItems` ids in the node's own view (`:369`). Add a unit test of the id format per view. Verify with `just test-crate platynui-provider-java` and `cargo check -p platynui-provider-java --tests`.

## 10. PlatynUI.core Locator, then the mock rebuild

- [ ] 10.1 Render `descendant::X[…]` on the `descendants` scope when `index`, `position` or custom predicates are set (design decision 9, `src/PlatynUI/core/locator.py:314-380`). Update the `to_xpath` docstring. Verify that the rendering tests of 4.3 pass (`uv run pytest tests/PlatynUI/test_locator.py`).
- [ ] 10.2 Run `just test-python`, which rebuilds the native module with the mock provider. Verify that 4.2 and the `test_adapter_factory.py` cases of 4.3 pass, and that the whole Python suite is green.
- [ ] 10.3 Run `just test-baremetal`. Verify that 2.1 passes, that every other mock suite stays green, `set_root_scope.robot:120-124` included, and that no suite depended on the mock's flat window copy. Then run `just build-native`, so that the real `Runtime` enumerates the desktop again.

## 11. Documentation

- [ ] 11.1 Update:
  - `dev-docs/architecture.md`:
    - §9.1–9.2 (`:701-731`): document order by construction, normalization only where it is not proven, per-context positional predicates, `(E)[n]` against `E[n]`, the bounded advance;
    - §5.4: an agent row with `agent/<pid>/<id>` and `agent/app/<pid>/<id>`, and the rule of one id per view;
  - `crates/xpath/docs/xpath20_coverage.md:55-56`, including the attribute-before-namespace deviation;
  - `dev-docs/python-library-design.md:2978`, with an English summary line at the top of this German document;
  - BareMetal's "Finding elements" (`src/PlatynUI/BareMetal/__init__.py:568-583`): `(//Button)[2]` against `//Button[2]`, and the idiom `(.//X[@Name="t"])[1]/*[n]` for a container known to be unique.

  Verify by reading, and with `just check`.

## 12. Verification

- [ ] 12.1 Run `just check`, `just test`, `just test-python` and `just test-baremetal`, then `just build-native`. Verify that everything is green.
- [ ] 12.2 On Windows, with the maintainer's go-ahead, run `just install-provider-java`, `just test-acceptance-windows` and `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - everything is green, including 5.1, 5.2 and the Swing suites that address cells by position;
  - there is no WARN or ERROR from PlatynUI.

  Measure again the numbers of 1.1. If the positional cell tests became noticeably slower, rewrite them to the idiom `(.//*[@Name="main-table"])[1]/*[n]`, rerun the Swing suites, and record the before and after times here with the decision.
- [ ] 12.3 CI runs the X11 and compositor lanes on push. Verify there that both lanes are green and their logs have no WARN or ERROR from PlatynUI, and record the run here.

## 13. Commit (only when the maintainer asks)

- [ ] 13.1 Commit in reviewable steps. Each step carries the tests it turns green, so each builds, passes lint and passes its tests on its own:
  1. the engine, with its XPath tests, the native mock tests of 4.2 and `document_order.robot`;
  2. the runtime, with 4.1;
  3. the Java agent ids, with 5.1 and 5.2;
  4. the Locator, with 4.3;
  5. the docs, and the suite rewrite of 12.2 if one was made.

  Subjects are at most 72 characters, with no `!`. The engine commit's body lists the behavior changes of the proposal for the release notes. The Java agent commit's body names the new ids of the application view.
