# Design

## Context

The proposal says why this change exists (see proposal.md, Why). The specs say what the results must be. This section records the state of the code that the decisions below depend on. Every line reference was checked on `main` at `65cd1c17`. Every "today" result was measured at `999ce95` with a scratch crate that depends on `crates/xpath`; since then `crates/xpath/src` has changed only in `Expr::is_context_dependent`, which the compiler does not use, and in when a trace record is compiled, so the results still hold. Each expression ran on `SimpleNode`, which has order keys, and on a keyless wrapper that delegates to `try_compare_by_ancestry`, as the real providers do. Keyed and keyless results were identical.

**Where order is lost.**

- The parser turns `//` into an explicit `descendant-or-self::node()` step (`crates/xpath/src/parser/mod.rs:1198-1211`, `:1256-1265`).
- The compiler decides after each step whether to normalize (`crates/xpath/src/compiler/mod.rs:543-585`):
  - after `child`, `self` and `attribute` it emits nothing (`:557`);
  - after `descendant*`, `following` and `following-sibling` it emits only `EnsureDistinct` (`:558-562`);
  - after `parent`, `ancestor*` and `preceding*` it emits `EnsureDistinct` and `EnsureOrder` (`:563-571`);
  - a filter-expression step, such as the union in `.//(A|B)[p]`, becomes an opaque `PathExprStep` followed by both (`:574-581`). `//(P|B)` still gives `[P1, B3, B1, B2, P2, B4, B5]`, because `EnsureOrder` cannot repair it (see Order repair).
- A child step over nested context nodes therefore concatenates their children in the order of their parents.
- The base of `(E)/steps` is never normalized (`:373-376`).

**Predicates.**

- The AxisStep opcode runs the step's predicates once over the concatenated output of all context nodes (`crates/xpath/src/engine/evaluator/mod.rs:675-682`, `:575-586`). `position()` and `last()` are therefore global.
- `PredicateCursor` (`engine/evaluator/cursors.rs:880-993`) already handles one stream correctly, including `last()` and early termination.
- The optimizer merges an `ApplyPredicates` into the preceding axis step unconditionally (`crates/xpath/src/compiler/optimizer.rs:64-127`, `:147-158`). So `(//X)[1]` and `//X[1]` compile to the same code.

**Minimization.** Three cursors drop context nodes whose results another context already covers (`cursors.rs:15-36`, `ContextMinCursor` `:467-527`, `ContextMinFollowingCursor` `:529-605`, `ContextMinFollowingSiblingCursor` `:607-656`).

- This is only valid when no predicate of the step is positional.
- `ContextMinFollowingCursor` keeps the ancestor and drops the nested context, which loses results.

**Order repair.**

- `EnsureOrderCursor` (`cursors.rs:1555-1687`) holds one item of lookahead and cannot take back what it has emitted.
- Its adjacent-swap branch is dead code.
- The sort comparators in `engine/evaluator/set_ops.rs:15-81`, `:158-209` map errors and missing siblings to `Equal`. `try_compare_by_ancestry` returns `Equal` for a node missing from its parent's list (`crates/xpath/src/model/mod.rs:93-97`). Such a comparison is not a total order, and Rust's sort may panic on it.

**Deduplication.** `DistinctCursor` (`cursors.rs:1508-1553`) is a hash set of order keys, with a linear list for nodes without keys: O(N²). It measured 15.0 ms keyless against 2.6 ms keyed for `count(descendant::*)` on 11,111 nodes.

**Runtime snapshot.**

- `RuntimeXdmNode` compares elements by runtime id plus order key, and attributes by owner id, namespace and name (`crates/runtime/src/xpath.rs:518-531`).
- The parent of an attribute is a fresh wrapper built from the provider node (`:597`). It has an unresolved parent link, so navigating from an attribute upward reads the provider again.
- The `xpath-snapshot` capability requires that a list of children is read at most once per query. It also requires that wrappers never form a strong cycle (`:422-430`).

**Keys.** Since `04d3aed` only the mock supplies order keys (`crates/provider-mock/src/tree.rs:175-205`); every real provider is keyless.

**Identity per view.**

- UI Automation and JAB scope runtime ids per view (`crates/provider-windows-uia/src/map.rs:230-253`; `dev-docs/architecture.md` §5.4).
- The Java agent does not. The backend lists each window flat (`crates/provider-java/src/agent/backend.rs:387-408`) and under an `AgentAppNode` (`:409-416`). `AgentAppNode::children` (`agent/app.rs:163-176`) builds new nodes whose id is `agent/{pid}/{element id}` (`agent/node.rs:224`), the same in both views.
- The hit-test chain hangs below an `AgentAppNode` (`backend.rs:500-519`).
- `SelectedItems` builds the same flat ids (`agent/node.rs:369`).
- The Inspector reveals a picked node by walking down its ancestors' runtime ids (`apps/inspector/src/viewmodel/async_tasks.rs:177-215`, `inspector_vm.rs:307-331`). With one id in two places, `reveal_node_cached` can land on either copy.

**First-match consumers.**

- `evaluate_first` and the runtime's `evaluate_single*` take the first streamed item (`evaluator/mod.rs:294-299`; `crates/runtime/src/runtime/evaluation.rs:85-96`, `:192-219`).
- BareMetal uses the first item for selector resolution, `Query only_first` and the waits (`src/PlatynUI/BareMetal/__init__.py:416`, `:1832`, `:1934`, `:2041`, `:2164`).
- PlatynUI.core's `find_one` uses it too (`src/PlatynUI/core/adapter_factory.py:104`).
- The CLI's `focus`, `pointer` and `query`, and the Inspector's search, also consume it.

**Measured first-match cost.**

- Setup: a keyless tree `W` → 50 `P` → 20 `B` (1,052 nodes) that counts the distinct lists of children read.
- Two or three lists suffice today for `//B`, `//B[1]`, `(//B)[1]`, `//P/B[1]` and `//W/P[@id='P0']`. This is cheap only because the order is wrong: the engine never looks into the preceding siblings' subtrees.
- The correct lower bounds are as follows. The oracle: the lists read are a subset of the result's ancestors, the result itself, and the nodes before it.

| Shapes | Lists read by a correct engine |
|---|---|
| `//B[@id='B0_3']`, `//P/B[@id='B0_3']`, `//(P|B)[@id='B0_3']` | 6 (`#doc`, `W`, `P0`, `B0_0`, `B0_1`, `B0_2`) |
| `//B`, `(//B)[1]`, `//B[1]`, `//P/B[1]`, `.//B[2]` from `W` | 3 |
| `//W/P[@id='P0']` | 2 |

**Tests.**

- `crates/xpath/tests/it` is one binary; each module must be declared in `tests/it/main.rs`.
- The crate's only dev-dependency is `rstest`.
- `crates/runtime/tests/xdm_release.rs:59-171` holds a lazy fake provider that counts listings.

## Goals / Non-Goals

**Goals:**

- Correct XPath 2.0 results for every path shape (the `xpath-evaluation` spec), on keyed and keyless models, with a sort that is a total order.
- Streaming stays the rule. A shape whose order the compiler can prove produces its first item within the oracle bound above. Only shapes whose order cannot be proven buffer their input.
- No extra provider reads. The sort reads no list of children that the snapshot has not already read, never reads an element's attribute list, and never rebuilds ancestors from an attribute.
- One node per view in every in-repo provider, so identity and tree position agree.

**Non-Goals:**

- The numeric reading of `xs:untypedAtomic` in predicates (`evaluator/mod.rs:1765`), XPTY0018/XPTY0019 checks, and the XPath overflow and slicing bugs found during the lint adoption. They stay out of scope.
- The quadratic sibling navigation over wide lists (`next_sibling_in_doc`, `cursors.rs:719-731`). It predates this change and is only recorded here. Decision 6 makes sure the sort does not add a second quadratic path.
- A different desktop order on the mock. The mock keeps listing its applications before its flat windows; the release notes say what that means.
- XDM's namespace-before-attribute order. Both the keys of `SimpleNode` (`crates/xpath/src/model/simple.rs:246-259`) and `try_compare_by_ancestry` (`model/mod.rs:84-88`) put attributes first. The new sort keeps that order, so keyed and keyless results agree, and the deviation is documented.

## Decisions

### 1. Classify predicates when they are lowered, and carry the flag in the IR

A predicate is **non-positional** only when it is provably boolean or node-valued and does not use `position()` or `last()` in its own focus. That covers:

- comparisons, `and`, `or` and `not`;
- the boolean built-ins (`contains`, `starts-with`, `ends-with`, `matches`, `exists`, `empty`, `boolean`, `true`, `false`);
- a string literal;
- a path whose last step is an axis step.

Everything else is **possibly positional**: numeric literals, variables, arithmetic, other function calls, and a path ending in a filter-expression step. `position()` and `last()` inside a nested step's predicate have their own focus and do not count. Inside `for`, `some` and `every` bodies they count.

The classifier uses the focus rule of `Expr::is_context_dependent` (`crates/xpath/src/parser/ast.rs:115`, since `8871d04a`): operands, conditions, bindings, function arguments and the bodies of `for`, `some` and `every` evaluate in the predicate's focus, while a nested predicate and every later step of a path have their own. It sits next to that method on the AST and shares its walk, so the rule lives in one place and the two cannot drift apart. The compiler asks it when it lowers a predicate.

The flag is stored per predicate in the IR, so each predicate becomes an instruction sequence plus a flag. This touches `parser/ast.rs`, `compiler/mod.rs:457-466`, the IR types in `compiler/ir.rs:96-104`, and the IR's `Display` (`:415-421`) and `fmt_with_indent` (`:495-508`). A conservative "possibly positional" is always correct.

*Alternatives:* an IR scan like `instr_seq_uses_last` (`cursors.rs:1073-1099`) cannot tell that a predicate yields a number. Treating every predicate as positional is correct, but it would disable minimization, pushdown and the `//T[p]` rewrite for every locator. A walk of its own in the compiler would repeat the focus rule of `is_context_dependent`, and the two could drift apart.

### 2. Evaluate possibly-positional step predicates per context node, and minimize only without them

When a step has at least one possibly-positional predicate, the step's whole predicate chain runs once per context node. It runs over that context's axis stream: document order on forward axes, nearest first on reverse axes, as `NodeAxisCursor` already yields them. The per-context results are then combined in document order (decisions 4–6). `PredicateCursor` is reused unchanged per context, together with its `last()` and early-termination paths.

Context minimization is allowed only when every predicate of the step is non-positional. Without that restriction, `//B/following::B[1]` would keep one context and return `[B2]` instead of `[B2, B3, B4, B5]`, and `//X/descendant::B[1]` would return one node instead of two. The rule applies to all three minimizing cursors.

*Alternatives:* rewriting `X[n]` into `[count(preceding-sibling::X) = n-1]` has no `last()`, costs O(siblings²) and does not cover reverse axes. Keeping global positions was overruled by the maintainer.

### 3. Push down only non-positional predicates, then rewrite `//T[p]` and `.//(A|B)[p]` into one descendant step

For `(E)[p1][p2]…`, the optimizer moves only the leading run of non-positional predicates into E's last axis step:

- `(//T)[@a][1]` moves `[@a]` and keeps `[1]` on the whole sequence;
- `(//T)[1][@a]` moves nothing.

Today the whole list moves or none of it (`optimizer.rs:96-121`).

After pushdown, a `descendant-or-self::node()` step without predicates, followed by `child::T[p…]` with only non-positional predicates, becomes `descendant::T[p…]`. The two are equivalent exactly under that condition. `descendant::A[q]/child::T[p]` is not rewritten: from a context that itself matches `A[q]` the result would change (verified: `[T1, T2]` instead of `[T2]` on `A0:[T1, A1:[T2]]`). The optional pushdown of a positional predicate into a step whose input is statically one node, as in `(child::X)[1]`, is left out: it saves little.

The union idiom `.//(A|B)[p…]` gets the same rewrite. The robot-test-style skill recommends it for windows (`.//(Frame|Window)[@Name=…]`). Three conditions must hold:

- a filter-expression step follows a `descendant-or-self::node()` without predicates;
- its expression is a parenthesized union whose operands are single `child::` steps with name tests and no predicates;
- every predicate on it is non-positional.

Then the step becomes `descendant::*[self::A or self::B][p…]`. The two are equivalent exactly under those conditions: `//(P|B)[1]` is the first P or B of every parent (`[P1, B1, B4]`), while `descendant::*[self::P or self::B][1]` is the first overall (`[P1]`).

The optimizer sees such a step only as an opaque `PathExprStep` (`compiler/mod.rs:574-581`). So the compiler rewrites it while it lowers the path, where the union is still visible on the AST. It covers both forms of `//`: the explicit step of `.//` and `A//`, and the `//` at the start of a path. The rewrite comes before the optimizer, so pushdown applies to its result: `(//(A|B))[@a]` becomes one descendant step as well.

*Alternatives:*

- Removing pushdown entirely loses streaming for `(//X)[@a='x']`.
- The triage's rewrite to `descendant::T[p][parent::A[q]]` is wrong, as shown above.
- Rewriting the union in the optimizer would mean recognizing it inside the instructions of a `PathExprStep`. On the AST it is one pattern.

### 4. Track what the compiler knows about a node stream, and normalize only where order is not proven

Through a path, the compiler tracks four properties of the node stream:

- **single:** at most one node;
- **ordered:** in document order;
- **distinct:** each node once by position. Identity is the business of `EnsureDistinct` (decision 8);
- **peer:** no node is an ancestor of another.

The rules:

| Step | Input | Result |
|---|---|---|
| start at the root or the context item | — | single |
| `self` | any | unchanged |
| `attribute` | ordered | ordered, distinct, peer |
| `child` | single, or ordered and peer | ordered, distinct, peer |
| `child` | ordered, not peer | ordered child merge (decision 5); ordered, distinct |
| `descendant*` without positional predicates | ordered | minimized, then `EnsureDistinct`; ordered, distinct |
| `following` without positional predicates | ordered | minimized (decision 7), then `EnsureDistinct`; ordered, distinct |
| `following-sibling` | single | ordered, distinct |
| `parent`, `ancestor*`, `preceding-sibling` | single | reversed buffer; ordered, distinct |
| `preceding` without positional predicates | ordered | last context only, reversed; ordered, distinct |
| every other case of these axes, including positional predicates on `descendant*`, `following` or `preceding` from more than one context | — | normalize |
| filter-expression step that decision 3 does not rewrite; `(E)/…` whose base is a variable, a sequence or a function call | — | normalize |
| `(E)[p]` | — | E's properties; a literal `[1]` makes it single |
| `union`, `intersect`, `except` | — | ordered, distinct |

"Normalize" emits one normalization op (decision 6) in place of today's `EnsureDistinct` + `EnsureOrder` pair, and it deduplicates by identity itself. The streaming `EnsureDistinct` stays where it is today, after `descendant*` and `following`, where minimization leaves no two items at one position, but a model may still give two positions one identity. Child steps stay without it, as today. This table replaces `compiler/mod.rs:543-585` and covers the `PathFrom` base (`:373-376`).

*Alternatives:* always sorting would make `evaluate_single` read the whole tree for every query. Keeping today's table is wrong for child and following-sibling steps over nested input.

### 5. An ordered child merge with a bounded advance keeps the first match cheap

A child step over ordered, non-peer input is the shape `//T[n]`, `//A/T[p]` and `.//*[@Name='t']/*[3]/*[k]` take after decision 3. It runs as a merge that emits in document order. It keeps a stack of contexts, each an ancestor of the next. For each context it keeps the context's selected children, with the per-context predicates of decision 2 applied lazily. It emits the head `h` of the top context once no pending context can precede `h`.

A context precedes `h` only if it lies in the subtree of one of `h`'s preceding siblings. To find out without reading past `h`, ordered cursors get a **bounded advance**: *the next item if it precedes a given node, otherwise nothing, leaving the item pending.*

- The descendant walkers answer it by walking until they reach the given node. Every list they read then belongs to a node before it.
- The child merge answers it for its own consumers by locating its candidate against the given node through ancestry and the cached lists. That makes chains of child steps bounded too.
- A cursor without the method answers with a plain pull. That is correct, but may read further.

With this, each shape in the first-match requirement reads only what the oracle allows. The runtime fake's `//Button` reads `root`, `root/0` and `root/0/0`.

*Alternatives:*

- A fused pre-order walker for `//` plus child chains, as the dossier first proposed. It needs its own pattern matching in the compiler, and the merge covers the same shapes generically.
- A merge without the bounded advance. It has to pull the next context to emit anything, and for `//P/B[@id='B0_3']` it would read all 20 lists of `P0` and more instead of 6.
- A sort, which would read all 1,052.

### 6. One normalization cursor over a total order

The normalization op buffers its input, removes duplicates by identity (decision 8), sorts and emits. It replaces `EnsureOrderCursor`, and the set operations use the same order.

- **All nodes keyed:** sort by key.
- **Otherwise:**
  - Each node gets, once, the path of sibling indices from its root.
  - An element's index is its position in its parent's `children()`. It comes from the snapshot's cached list, and each parent's list is indexed once per sort through the identity hint.
  - Element children rank after the parent's attributes and namespaces without reading `attributes()`, because attributes and namespaces always come first.
  - An attribute sorts directly after its owner, among its owner's attributes by (namespace URI, local name). That order is stable and implementation-dependent, as XDM allows. The keyed order of `SimpleNode`, which follows its list, may differ, so keyed-versus-keyless test rows avoid comparing several attributes of one owner.
  - An attribute reaches its owner through a weak link to the element wrapper that listed it. `AttributeData` gains that link, and `parent()` uses it while it lives, falling back to today's rebuild. Being weak, it keeps the snapshot's wrappers free of strong cycles, so the drop-count tests of `xdm_release.rs` still hold.
- **A node missing from its parent's list** gets a slot after the known siblings, in arrival order. The order stays deterministic and the sort cannot panic.
- **Nodes of different roots** keep the order in which their roots first appeared, without an error. XPath calls that order implementation-dependent. Today the sorts map the root error to `Equal`, so no expression that works now will start failing.
- **Atomic values** pass through unchanged. This keeps the atomic-order test (`crates/xpath/tests/it/evaluator_path_filter_expr.rs:40-53`) and the non-conformant `(1, 2, 3)/xs:integer(.)` extension (`:72`) working.
- **Cancellation:** the cursor checks the cancellation flag while it drains its input, as streaming cursors do. The Inspector's cancellable search depends on it (`apps/inspector/src/viewmodel/async_tasks.rs:337`).

*Alternatives:*

- Keeping the one-item-lookahead cursor: it cannot be made correct.
- Keeping `node_compare` and `try_compare_by_ancestry` as comparators: they are not total.
- Ranking children in "attributes, then namespaces, then children" order through `parent.attributes()`: that reads about 1,050 COM properties per element on UIA (`crates/runtime/src/xpath.rs:628`).
- A k-way merge of per-context streams: it does not help reverse axes. It may come later as an optimization.

### 7. Minimize `following::` by the earliest subtree end

For `following::` without positional predicates, the minimizing cursor keeps the context whose subtree ends first. A later context inside the kept one replaces it, and with ordered input the scan stops at the first context after the candidate's subtree. This holds because `following(y) ⊇ following(x)` when `y` ends no later than `x`. The output is ordered and distinct. This fixes `//a/following::c` on `r:[a1:[a2,c1]]`.

*Alternative:* normalizing after concatenating all contexts' outputs. It is correct, but gives up streaming and costs O(N) per context.

### 8. Identity: deduplicate by identity in O(1), and make the Java agent one node per view

**Deduplication.**

- Deduplication by identity stays where it is today: in the streaming `EnsureDistinct` after `descendant*` and `following`, and inside every normalization.
- `XdmNode` gets an optional identity hint, a `u64` that equal nodes share. It defaults to `None`, so existing implementors keep compiling.
- `RuntimeXdmNode` hashes what its equality compares: runtime id and order key for elements, runtime id for the document, owner id plus name for attributes. `SimpleNode` hashes its pointer.
- `DistinctCursor` and the sort's index lookup use the hint. Nodes without a hint keep today's linear fallback.
- The hint is additive. Making `Hash` a supertrait would break every implementor for no gain.

**The Java agent.**

- The agent backend scopes runtime ids per view, as UI Automation and JAB do: `agent/<pid>/<id>` in the flat view, `agent/app/<pid>/<id>` under `app:Application`.
- The view is fixed where a node is created: the backend's sweep is flat, while `AgentAppNode::children` and the hit-test chain are in the application view. It passes to every descendant.
- `SelectedItems` uses the reporting node's view.
- Each view then has its own nodes, which is what the Inspector's reveal needs. Identity equals tree position for every in-repo provider.
- Only host-side Rust changes. The agent JAR and the agent protocol stay the same, so the exact-version handshake is unaffected.
- `live_two_hosts_share_one_agent_and_agree_on_identity` still holds, because both hosts read the flat view.

*Alternatives:*

- Keeping one id for both views: the rewrite of decision 3 would then change `count(//Window)` for agent windows from 2 to 1, while the copy under the application node comes back through the child step. Identity dedup and tree position would disagree.
- Dropping identity dedup after descendant steps: correct only while identity equals position, which is a convention, not a contract of `UiNode`.

### 9. PlatynUI.core's Locator renders `descendant::` when it counts

On the `descendants` scope, a locator with an `index`, a `position` or custom predicates renders `descendant::X[…]` instead of `.//X[…]`. Examples: `descendant::Button[2]` and `descendant::Button[position()=3][1]`.

A locator is resolved against one parent (`src/PlatynUI/core/adapter_factory.py:89-107`), so this counts over all descendants in document order, which is what `index` and `position` mean to users. The rendering also streams through decision 3's descendant walker.

Plain locators keep `.//X[…]`. Of the 23 `.//` assertions in `tests/PlatynUI/test_locator.py`, only `test_index_and_position` (`:117-119`) changes; the raw `position()=2` case (`:111-114`) checks substrings and still passes. The `children`, `root` and reverse scopes are a single step from one parent and are unchanged. `ancestor::Pane[1]` stays the nearest ancestor. A `path` or an explicit `axis` is rendered as given.

*Alternative:* `(<path>)[index]`. It changes the reverse scopes (`(ancestor::*)[1]` is the root, not the parent) and leaves `position` inside a step.

### 10. Docs and the parenthesized-index idiom

The change updates:

- `dev-docs/architecture.md` §9.1–9.2 (`:709-739`) and the §5.4 table (`:222-234`): a Java agent row and "one id per view".
- `crates/xpath/docs/xpath20_coverage.md:55-56`.
- The doc comments at `optimizer.rs:11-51` and `:129-158`, `evaluator/mod.rs:220-244` (`(//item)[1]` as the first-item example), `ir.rs:99-104` and `cursors.rs:529`.
- `dev-docs/python-library-design.md:2948` and `:2977`, the Locator's scope mapping and its `[N]` suffix. The document is German; its English summary (`:3-7`) describes the whole document and needs no new line.
- BareMetal's "Finding elements" (`src/PlatynUI/BareMetal/__init__.py:800-820`). It explains `(//Button)[2]` against `//Button[2]`, and recommends `(.//X[@Name="t"])[1]/*[n]` over `.//X[@Name="t"]/*[n]` when the container is known to be unique (see the risk on wide lists).

## Risks / Trade-offs

- **[`.//*[@Name="main-table"]/*[544]` reads every preceding cell]**
  - Document order requires reading the lists of the 543 cells before cell 544, because a nested `main-table` inside one of them would come first. On the agent and JAB that is one call per cell, for every `Get Attribute Value` (`tests/acceptance/swing/native_attributes.robot:66-68`, `agent_table.robot:37-132`).
  - → Measure the suites' keyword times on the Swing fixture before and after. If the regression is noticeable, rewrite the repo's suites to `(.//*[@Name="main-table"])[1]/*[n]`, which reads only the table's list, and document the idiom (decision 10). The engine cost is the price of a correct answer, and it is accepted.
- **[Shapes that must sort read all their input before the first result]**
  - These shapes are sibling, parent and reverse axes from several contexts, filter-expression steps that decision 3 does not rewrite (such as `.//(A|B)[1]`), and `(E)/…` bases. `evaluate_single` on them reads every matching context.
  - → They already wait for more than one item today. The union idiom `.//(Frame|Window)[@Name=…]` streams, because decision 3 rewrites it.
- **[User suites that wrote `//X[n]` meaning "n-th overall" change silently]** They may now match several elements, and keywords take the first. → A release note, the BareMetal doc line, and the new BareMetal suite that shows both forms.
- **[The mock resolves the other copy of a window]**
  - `//control:Window[@Name=…]` on the mock now finds the copy under `Mock Application` first, because the mock lists applications first (`crates/provider-mock/src/provider.rs:34-40`).
  - Reading found no mock suite or Python test that depends on the flat copy's parent chain. `tests/BareMetal/set_root_scope.robot:119-127` keeps its counts.
  - → The full `just test-python` and `just test-baremetal` runs are part of verification.
- **[The sort must stay total]** → It never calls the old comparators. Unit tests cover a keyless node missing from its parent's list, and nodes of two roots.
- **[Wide lists get a second quadratic path]** → Each parent's list is indexed once per sort through the identity hint. Nodes are not looked up one by one.
- **[Scope: several substantial parts land together]** The parts are the classification, per-context predicates, the property table, the merge with bounded advance, the normalization cursor, the identity hint and the per-view agent ids. → Tests come first and are grouped per part. `tasks.md` orders the parts so that each group turns its own tests green.
- **[The runtime snapshot follows the `xpath-snapshot` rules]** One read per list and query, and no strong cycles, constrain decisions 6 and 8. → The attribute's weak owner link and the identity hint are the only changes to `crates/runtime/src/xpath.rs`. `xdm_release.rs` stays green, with the read-once test extended to a sort over attributes and elements.

## Migration Plan

- **Kind of change.** This is a behavioral change for every provider, with no configuration and no data migration. The release notes take the list from the proposal.
- **Rebuild.** The native module needs a rebuild (`just build-native`), because the engine, the runtime and the Java provider are linked into it. The Java agent JAR and `packages/provider-java` do not change, so the three-part agent version stays as it is.
- **Order of landing.**
  1. The engine with its tests.
  2. The runtime (the identity hint and the attribute link).
  3. The Java agent ids.
  4. The Locator.
  5. The docs.

  Each lands as its own commit that passes lint. The changes this one waited for are archived: `xdm-snapshot-release` (2026-09-28), `application-process-attributes` and `snapshot-validity` (2026-09-29). The runtime commit extends `xdm_release.rs` and keeps its existing tests green. Open changes that edit other functions of the agent's files are listed in the proposal (Coordination); whichever lands second rebases.
- **Rollback.** Revert the commits and rebuild the native module. Suites that adopted `(//X)[n]` keep working after a rollback, because before this change `(//X)[n]` and `//X[n]` evaluated to the same thing.

## Open Questions

None.
