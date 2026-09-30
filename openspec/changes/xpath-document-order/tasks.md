# Tasks

Rules for every task:

- No test or measurement uses the taskbar, the shell or applications that Windows ships.
- Queries used for measurements are scoped to a repository test app, for example `count(.//*)` with the root `/app:Application[@ProcessId=${pid}]`. They are never desktop-wide.
- The Windows acceptance lane takes over the pointer and the keyboard of the desktop it runs on. On a real desktop, ask the maintainer before each run. In the Windows VM it takes over only the VM's desktop and needs no asking, and neither do the Linux lanes, which run in a display of their own.
- When the Windows lane runs in the VM, run no host lane or build at the same time. The VM shares the host's CPU, and the timings of both would be skewed.

## 1. Before the change

- [x] 1.1 Record the baseline on Windows, with the maintainer's go-ahead:
  1. `just install-provider-java`.
  2. `just test-acceptance-windows --profile real-windows run --suite AgentTable --suite NativeAttributes`. Suite names go without spaces: `just` passes its arguments on unquoted, and Robot Framework matches names without spaces.
  3. From `results/output.xml` (`uv run --no-sync robotcode results`), note the keyword times of the tests that address cells by position (`agent_table.robot:37-132`, `native_attributes.robot:49-68`).
  4. The time of `Query    count(.//*)    only_first=${True}` with the root set to the fixture's `/app:Application[@ProcessId=${pid}]`.

  Record the numbers here.

  Recorded on 2026-09-30 in the Windows VM, on `main` at `75a9bd3b`. 18 of 18 tests passed; the Robot run took 30.6 s of the recipe's 254 s. Step 4 ran as two throwaway suites in `tests/acceptance/swing/`, deleted afterwards. Each launches the fixture through the suite setup of `testapp_agent.resource` or `testapp.resource` and runs the query five times. The suites, the run's `output.xml` and the script that extracts these times are kept in `results/xpath-document-order-baseline/`, which git ignores.

  Times per keyword call. The first call of a test is the slowest, so it is listed on its own:

  | Selector | Backend | Calls | First | Others |
  |---|---|---|---|---|
  | `count(.//*[@Name="main-table"]/*)` (wait) | agent | 1 | 372 ms | — |
  | `.//*[@Name="main-table"]/*[n]`, rows 1–4 | agent | 4 | 28 ms | 30–35 ms |
  | `…/*[3]/*[n]`, cells of row 3 | agent | 12 | 14 ms | 10–17 ms |
  | `…/*[91]`, `…/*[91]/*[1]` | agent | 7 | 9 ms | 21–28 ms |
  | `…/*[3]/*[1]`, `…/*[2]/*[3]` | agent | 6 | 26 ms | 21–28 ms |
  | `.//*[@Name="main-table"]`, table attributes | JAB | 6 | 299 ms | 73–89 ms |
  | `…/*[9]` | JAB | 5 | 99 ms | 71–87 ms |
  | `…/*[14]` | JAB | 3 | 94 ms | 70–71 ms |
  | `…/*[544]` | JAB | 3 | 1,006 ms | 134–136 ms |
  | `count(.//*)` from the application node, 744 elements | agent | 5 | 432 ms | 202–284 ms |
  | `count(.//*)` from the application node, 652 elements | JAB | 5 | 2,359 ms | 1,933–2,005 ms |

## 2. Tests first — Robot Framework on the mock

Follow the `robot-test-style` skill.

- [x] 2.1 Add `tests/BareMetal/document_order.robot` with the six scenarios of `baremetal-selector-resolution` ("Several matches resolve to the first in document order").
  - Import: `Library    PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}`, with the root set to `//control:Window[@Name="Operations Console"]`, in the style of `selector_resolution.robot`.
  - The suite documentation explains `(.//X)[n]` against `.//X[n]`.

  Verify that all six fail today: `just test-baremetal --suite DocumentOrder`.

## 3. Tests first — XPath engine

- [x] 3.1 Fixtures in `crates/xpath/tests/it/common/mod.rs`:
  - a keyless wrapper around `SimpleNode` (no `doc_order_key`, `compare_document_order` by ancestry);
  - a keyless wrapper that records whose `children()` were read;
  - builders for the spec trees `r:[A:[X1],X2]`, `r:[X1:[X2:[Y1],Y2]]`, `r:[a1:[a2,c1]]`, `W:[P1:[B1,B2],B3,P2:[B4,B5]]`, `r:[a,b,c]`, `r:[a:[b1,b2],c]`, `r:[x1,x2,y1,x3,y2]`, `r:[X1:[B1,X2:[B2]]]`;
  - the wide tree `W` → 50 `P` → 20 `B`;
  - a keyless tree with one node missing from its parent's list;
  - two separate documents.

  Declare every new test module in `tests/it/main.rs` (`:1-6`). Verify that `just test-crate platynui-xpath` builds.
- [x] 3.2 Add `document_order.rs`. It is a table over the scenarios of "A path returns its nodes in document order without duplicates", and each row runs on the keyed and on the keyless model. Include the missing-node and two-roots scenarios; the atomic scenario already exists at `evaluator_path_filter_expr.rs:40-53`. Verify that the rows marked "before this change" fail today, and that the `(//B)[last()]` and `//B/..` controls pass.
- [x] 3.3 Add `positional_predicates_per_step.rs` with the scenarios of "A positional predicate in a step counts per context node" and "A predicate on a parenthesized expression counts over the whole sequence", keyed and keyless. Verify that the rows with a "before this change" note fail today, `//B[3]` included. Also verify that `//B[0]`, `//B[1.5]`, `(//B)[6]`, `//(P|B)[1]`, `//(P|B)[last()]`, the atomic-sequence row and `//B[(1, 2)]` → `FORG0006` pass.
- [x] 3.4 Add `document_order_oracle.rs`:
  - It enumerates every ordered tree of up to six element nodes labelled `a` or `b`.
  - It compares, on the keyed and keyless models, `//a`, `//a[1]`, `//a[last()]`, `(//a)[2]`, `//a/b`, `//a/b[1]`, `//*/following-sibling::a`, `//a/following::b`, `//a/following::b[1]`, `//a/preceding::b[1]`, `//a/ancestor::*[1]`, `//a/descendant::b[1]`, `//(a|b)[1]`, `//(a|b)[b]` and `//*/..` against a naive evaluator written in the test itself. The naive evaluator lists each axis, and each union of child steps, from each context by pre-order index, applies predicates per context, then sorts and deduplicates, and shares no code with the engine.

  Verify that it fails today and names the first failing tree and expression.
- [x] 3.5 Add `first_match_reads.rs`, using the recording wrapper on the wide tree. For the first item of `//B[@id='B0_3']`, `//B`, `(//B)[1]`, `//B[1]`, `//P/B[1]`, `//P/B[@id='B0_3']`, `//W/P[@id='P0']`, `.//B[2]` from `W` and `//(P|B)[@id='B0_3']`, assert three things:
  - the item equals the first item of the full result;
  - every list read belongs to the item, one of its ancestors, or a node before it;
  - the count stays within the design's bounds: 6, 3, 3, 3, 3, 6, 2, 3 and 6.

  Also assert that the first item of `//B/..` on `W:[P1:[B1,B2],B3,P2:[B4,B5]]` is `W`, that a full `//B` reads 1,052 lists, and that a full `(//P[@id='P0'])[1]/*[20]` reads only the lists of the document, `W` and `P0`. Verify that the order assertions fail today where the order is wrong (for example `//B` gives `B3` first on the small tree).
- [x] 3.6 Add two plan tests:
  - `compiler_predicate_rewrites.rs` (design decisions 1 and 3) asserts:
    - pushdown of the leading non-positional run only: `(//T)[@a][1]` moves `[@a]` and keeps `[1]`, and `(//T)[1][@a]` moves nothing;
    - `[1]`, `[$n]`, `[count(x)]` and `[position() < 3]` never move into a step;
    - `[@a]`, `[@a='x']` and `[contains(@a,'x')]` move;
    - `//T[@a]` and `.//T[@a]` compile to one `descendant::T` step, while `descendant::A[@q]/T[@p]` keeps its child step;
    - `//(A|B)` and `.//(A|B)[@a]` compile to one `descendant::*` step whose first predicate is `self::A or self::B`, and `(//(A|B))[@a]` moves `[@a]` into it;
    - `.//(A|B)[1]` and `.//(A|B/C)[@a]` keep their filter-expression step.
  - `compiler_document_order_plan.rs` (design decision 4) asserts:
    - no normalization op for `//B[@id='x']`, `Window[@Name='x']//Button[@Name='y']`, `//Window/Button[@Name='OK']`, `.//B[2]`, `(//B)[1]`, `.//*[@Name='t']/*[3]/*[2]` and `.//(A|B)[@a]`;
    - one normalization op for `//B/..`, `//B/ancestor::*`, `(//c, //a)/self::*`, `//*/following-sibling::*` and `.//(A|B)[1]`.

  Each names the IR it checks: the predicate flag of decision 1, and the normalization op of decision 4. Verify that neither compiles yet. The first compiles once 6.1 carries the flag, the second once group 7 adds the op. `tests/it` is one binary, so each module is declared in `tests/it/main.rs` only by the task that makes it compile (6.1 and 7.2); until then the other tests keep running.
- [x] 3.7 Update the tests that assert the old behavior:
  - In `evaluator_more.rs:158-181`, the helper `:41-60` counts position per section, so the count becomes 48 instead of 53.
  - The shape tests of `compiler_paths_predicates.rs` (`:8-14`, `:22-36`, `:38-49`, `:63-68`, `:164-194`, `:196-222`, `:259-272`) move to the new plan and predicate type.
  - The unit tests of `crates/xpath/src/compiler/optimizer.rs:312-396`: positional literals no longer merge.
  - The comments in `evaluate_first.rs:98-99` and `optimizer_integration.rs:51-61`.

  Verify that each of them fails or does not compile until group 6 lands.

## 4. Tests first — runtime, Python and PlatynUI.core

- [x] 4.1 In `crates/runtime/tests/xdm_release.rs`, next to its lazy fake:
  - the first item of the evaluation stream for `//Button` is `root/0/0/0`, and the lists read are only those of `root`, `root/0` and `root/0/0`;
  - `(//Button)[2]` is `root/0/0/0/0`;
  - extend `each_list_is_read_once_per_query` with a union that mixes attributes the fake serves and elements, so the sort has to place attributes.

  With the mock (`crates/runtime/src/runtime/evaluation.rs` tests), the names of `(//control:Window[@Name='Operations Console'])[1]//item:TreeItem` come out in document order. Verify with `just test-crate platynui-runtime` that the order and read assertions fail today and the drop-count tests stay green.
- [x] 4.2 In `packages/native/tests/test_mock_tree_content.py`, add:
  - `evaluate('(//control:Window[@Name="Operations Console"])[1]//item:TreeItem')` gives the names in document order;
  - the same path with `[2]` on the last step gives Metrics, Reports and Monatlich;
  - `evaluate_single` on the same path with `[@item:Name!="Dashboard"]` gives Overview; the mock names its tree items in `item:Name`.

  They fail until the mock rebuild in 10.2.
- [x] 4.3 In `tests/PlatynUI/test_locator.py`, add the rendering scenarios of `core-locator` and change `test_index_and_position` (`:117-119`) to `descendant::Button[position()=3][1]`. In `tests/PlatynUI/test_adapter_factory.py`, next to the mock-backed `RuntimeAdapterFactory` tests, add `find_one` with `index=2` (Overview) and `index=8` (None), and `find_all` in document order. Verify with `uv run pytest tests/PlatynUI/test_locator.py` that the rendering tests fail today.

## 5. Tests first — Java agent ids per view

- [x] 5.1 In `crates/provider-java/tests/live_fixture.rs`:
  - Add an ignored live test for the scenarios of "Agent runtime ids are scoped per view": two nodes and two id forms for the fixture window, the prefixes of one button in both views, and `SelectedItems` per view.
  - Change the hit-test check of `live_agent_serves_table_cells_the_bridge_cannot` (`:1160-1176`). It compares the picked cell with a cell of the flat window. It must compare with the same cell reached through the fixture's `app:Application` node, and check that the pick's ancestors lead to that node.
  - Keep `live_two_hosts_share_one_agent_and_agree_on_identity` unchanged.

  Verify with `cargo check -p platynui-provider-java --tests`. The tests run in the Windows lane (12.2).
- [x] 5.2 In `tests/acceptance/swing/agent_table.robot`, add "The Window Is One Element Per View". It evaluates the union of the flat window and the window under its `app:Application` node, both scoped to the fixture's title and `@Technology="JavaAgent"`, and asserts two elements with different `RuntimeId`s. It fails today with one element; verify in 12.2.

## 6. Engine — predicates

- [x] 6.1 Classify each predicate when it is lowered (design decision 1), with a classifier on the AST next to `Expr::is_context_dependent` (`crates/xpath/src/parser/ast.rs:115`) that shares its focus walk. Carry the flag in the IR, and print it in `Display` and `fmt_with_indent`. Add unit tests for the classifier: the non-positional and possibly-positional cases of the decision, `position()` inside a nested step and inside `for`, `some` and `every`. Verify with `just test-crate platynui-xpath` that the classifier tests pass, that `parser_context_dependence.rs` stays green, and that the crate builds.
- [x] 6.2 Evaluate a step's predicates per context node when one of them may be positional, and allow the three minimizing cursors only when none is (design decision 2). Verify that the per-context rows of 3.3 whose result does not depend on order now pass, for example `count(//B[1])` and `//B[3]`.
- [x] 6.3 Push down only the leading non-positional predicates, and rewrite a predicate-free `descendant-or-self::node()` followed by a non-positional `child::T[…]` into `descendant::T[…]` while the path is lowered, so that pushdown extends the result (design decision 3). Correct the doc comments at `optimizer.rs:11-51` and `:129-158`. Verify that the pushdown and `//T[p]` rows of `compiler_predicate_rewrites.rs` (3.6) and the updated unit tests of 3.7 pass.
- [x] 6.4 Rewrite `.//(A|B)[p]` into `descendant::*[self::A or self::B][p]` while the path is lowered, under the conditions of design decision 3. Cover both forms of `//`: the explicit `descendant-or-self::node()` step and the `//` at the start of a path. Verify that the union rows of 3.2 now pass, that the union rows of 3.3 still pass, and that `compiler_predicate_rewrites.rs` passes in full.

## 7. Engine — order

- [x] 7.1 Add the optional identity hint to `XdmNode` (default `None`), implement it for `SimpleNode` and the test wrappers, and let `DistinctCursor` use it with the linear fallback kept (design decision 8). Add a unit test that deduplicates hinted and unhinted nodes. Verify with `just test-crate platynui-xpath`.
- [x] 7.2 Add the normalization cursor with the total order of design decision 6:
  - by keys when all nodes are keyed;
  - otherwise by sibling-index paths built once per parent list through the hint;
  - attributes after their owner by (namespace URI, local name);
  - a deterministic slot for a node missing from its parent's list;
  - the order in which roots first appeared for different roots;
  - atomics unchanged;
  - the cancellation flag checked while draining.

  Replace `EnsureOrderCursor`, and use the same order in the set operations. Add unit tests for each case, and one where the sort sees no call to `attributes()`. Verify that they pass and that `evaluator_path_filter_expr.rs` stays green.
- [x] 7.3 Keep every step's output in document order and track single through a path, and emit normalization according to the table of design decision 4, including `(E)/…` bases, filter-expression steps and `(E)[p]`. Verify that 3.2 and the one-normalization-op rows of `compiler_document_order_plan.rs` (3.6) pass.
- [x] 7.4 Minimize `following::` by the earliest subtree end (design decision 7), with a unit test on `r:[a1:[a2,c1]]`. Verify that `//a/following::c` in 3.2 passes.
- [x] 7.5 Add the ordered child merge and the bounded advance (design decision 5), and answer the bounded advance in the descendant walkers and in the merge itself. Verify that 3.3, 3.4 and 3.5 pass, and that `compiler_document_order_plan.rs` (3.6) passes in full.
- [x] 7.6 Correct the remaining engine docs: `evaluator/mod.rs:220-244` (`(//item)[1]` as the first-item example), `ir.rs:99-104` and the comment at `cursors.rs:529`. Verify that `just test-crate platynui-xpath` is fully green, the tests updated in 3.7 included, and that `just clippy` is clean.

## 8. Runtime

- [x] 8.1 Implement the identity hint for `RuntimeXdmNode` from what its equality compares. Give `AttributeData` a weak link to the element wrapper that listed it, and let `parent()` of an attribute use it while it lives (design decisions 6 and 8). Update the cycle note at `crates/runtime/src/xpath.rs:422-430`. Verify with `just test-crate platynui-runtime` that 4.1 passes and that every existing test of `xdm_release.rs` stays green, the drop counts and the read-once rule included.

## 9. Java agent ids per view

- [x] 9.1 Give `AgentNode` its view, fixed where it is created and passed to every child in `AgentNode::children` (`agent/node.rs:251-281`):
  - flat in the backend's sweep (`crates/provider-java/src/agent/backend.rs:387-408`);
  - the application view in `AgentAppNode::children` (`agent/app.rs:163-176`) and in `build_chain` (`backend.rs:500-519`).

  Build the runtime id as `agent/<pid>/<id>` or `agent/app/<pid>/<id>` (`agent/node.rs:224`), and the `SelectedItems` ids in the node's own view (`:369`). Add a unit test of the id format per view. Verify with `just test-crate platynui-provider-java` and `cargo check -p platynui-provider-java --tests`.

## 10. PlatynUI.core Locator, then the mock rebuild

- [x] 10.1 Render `descendant::X[…]` on the `descendants` scope when `index`, `position` or custom predicates are set (design decision 9, `src/PlatynUI/core/locator.py:314-380`). Update the `to_xpath` docstring. Verify that the rendering tests of 4.3 pass (`uv run pytest tests/PlatynUI/test_locator.py`).
- [x] 10.2 Run `just test-python`, which rebuilds the native module with the mock provider. Verify that 4.2 and the `test_adapter_factory.py` cases of 4.3 pass, and that the whole Python suite is green.
- [x] 10.3 Run `just test-baremetal`. Verify that 2.1 passes, that every other mock suite stays green, `set_root_scope.robot:119-127` included, and that no suite depended on the mock's flat window copy. Then run `just build-native`, so that the real `Runtime` enumerates the desktop again.

## 11. Documentation

- [x] 11.1 Update:
  - `dev-docs/architecture.md`:
    - §9.1–9.2 (`:709-739`): document order by construction, normalization only where it is not proven, per-context positional predicates, `(E)[n]` against `E[n]`, the rewrites of `//T[p]` and `.//(A|B)[p]` into one descendant step, the bounded advance;
    - §5.4 (`:222-234`): an agent row with `agent/<pid>/<id>` and `agent/app/<pid>/<id>`, and the rule of one id per view;
  - `crates/xpath/docs/xpath20_coverage.md:55-56`, including the attribute-before-namespace deviation;
  - `dev-docs/python-library-design.md:2948` and `:2977`, the Locator's scope mapping and its `[N]` suffix; the English summary at the top of this German document already exists;
  - BareMetal's "Finding elements" (`src/PlatynUI/BareMetal/__init__.py:800-820`): `(//Button)[2]` against `//Button[2]`, and the idiom `(.//X[@Name="t"])[1]/*[n]` for a container known to be unique.

  Verify by reading, and with `just check`.

## 12. Verification

- [x] 12.1 Run `just check`, `just test`, `just test-python` and `just test-baremetal`, then the Linux lanes `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor`, which build the native module without the mock provider again. Verify that everything is green, and that the lanes' logs have no WARN or ERROR from PlatynUI.
- [x] 12.2 On Windows (the VM), run `just install-provider-java`, `just test-acceptance-windows` and `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - everything is green, including 5.1, 5.2 and the Swing suites that address cells by position;
  - there is no WARN or ERROR from PlatynUI.

  Measure again the numbers of 1.1, with the suites and the script kept in `results/xpath-document-order-baseline/`. If the positional cell tests became noticeably slower, rewrite them to the idiom `(.//*[@Name="main-table"])[1]/*[n]`, rerun the Swing suites, and record the before and after times here with the decision.
  Recorded on 2026-10-01 in the Windows VM, on `main` at `7c146cb9` with the change applied as a patch.

  - The first runs failed 4 tests of `egui/auto_activate.robot` and 2 keyboard tests of `native_attributes.robot`: a window outside the tests held the foreground, so the fixtures' windows could not come to the front. With it closed, the same suites passed 9 of 9 and 19 of 19.
  - The first measurement showed that comparing two nodes read the whole list of children of their parent: `…/*[9]` on the bridge enumerated all 600 cells of the table, 1,173 ms for its first call. A comparison now reads a list only as far as the later of the two nodes (design decision 6).
  - Final runs: the measurement with the old selectors (A) and with the idiom (B) passed 19 of 19 each, and the full lane passed 148 of 148, including 5.1 (`live_agent_runtime_ids_are_scoped_per_view`) and 5.2. Their logs have no WARN or ERROR.

  Times per keyword call in ms, the first call / the others:

  | Test | Backend | Baseline | A: old selectors | B: idiom |
  |---|---|---|---|---|
  | rows 1–4, after the count wait | agent | 28–35 | 26–157 | 28–38 |
  | cells of row 3 | agent | 28 / 10–17 | 247 / 26–45 | 26 / 7–12 |
  | row 91 and its first cell | agent | 9 / 21–28 | 129 / 113–216 | 9 / 21–28 |
  | a cell through its row by name | agent | 24 / 26 | 26 / 221 | 24 / 19 |
  | the rectangles of row 3 | agent | 10 / 9–23 | 23 / 26–195 | 7 / 7–30 |
  | `…/*[9]` | JAB | 99 / 71–87 | 111 / 71–77 | 94 / 68–84 |
  | `…/*[14]` | JAB | 94 / 70–71 | 96 / 82–89 | 75 / 63 |
  | `…/*[544]` | JAB | 1,006 / 134–136 | 2,319 / 838–842 | 1,013 / 125 |
  | `count(.//*)`, 744 elements | agent | 432 / 202–284 | 301 / 188–211 | 305 / 216–240 |
  | `count(.//*)`, 652 elements | JAB | 2,359 / 1,933–2,005 | 2,166 / 1,977–2,010 | 2,310 / 1,933–1,954 |

  Decision: with the old selectors the positional tests are noticeably slower, because a correct answer has to look inside the rows or cells before the one addressed. The Swing suites therefore address the table as `(.//*[@Name="main-table"])[1]`, which brings every positional test back to the baseline. The full walks did not change.
- [ ] 12.3 CI runs the X11 and compositor lanes on push. Verify there that both lanes are green and their logs have no WARN or ERROR from PlatynUI, and record the run here.

## 13. Commit (only when the maintainer asks)

- [x] 13.1 Commit in reviewable steps. Each step carries the tests it turns green, so each builds, passes lint and passes its tests on its own:
  1. the engine, with its XPath tests, the native mock tests of 4.2 and `document_order.robot`;
  2. the runtime, with 4.1;
  3. the Java agent ids, with 5.1 and 5.2;
  4. the Locator, with 4.3;
  5. the docs, and the suite rewrite of 12.2 if one was made.

  Subjects are at most 72 characters, with no `!`. The engine commit's body lists the behavior changes of the proposal for the release notes. The Java agent commit's body names the new ids of the application view.

  Step 5 went out as three commits: the suite rewrite (a test commit), the docs, and the record of these artifacts. Each of the steps 1 to 4 was checked on its own state: the engine and the runtime with the Rust workspace tests and Clippy, the engine also with the Python suite and the BareMetal mock suites, the Java agent ids with Clippy for Windows, and the Locator with the Python suite.
