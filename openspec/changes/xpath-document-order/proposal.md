# Proposal

## Why

The XPath engine returns the results of a path in the wrong order whenever matches sit at different depths, and every keyword that acts on "the first match" then acts on the wrong element, with no error. `//X`, `.//X` and `A//X` are lowered to `descendant-or-self::node()/child::X`. After a child step the compiler emits no ordering (`crates/xpath/src/compiler/mod.rs:552-571`), so the matches come out in the pre-order of their parents, not in document order. This holds for every provider, the mock included, and whether or not nodes carry order keys. Measured on `W:[P1:[B1,B2],B3,P2:[B4,B5]]`, `//B` gives `[B3,B1,B2,B4,B5]`, and `(//B)[1]` gives `B3`.

The same code has three more deviations from XPath 2.0:

- A positional predicate in a step counts over all context nodes together instead of per context node. `//B[1]` gives one node instead of the first B of every parent, and `(//X)[1]` and `//X[1]` compile to the same code (`crates/xpath/src/compiler/optimizer.rs:147-158`).
- `following::` from nested context nodes loses results: `//a/following::c` returns nothing on `r:[a1:[a2,c1]]`.
- The repair cursor for out-of-order streams cannot un-emit (`crates/xpath/src/engine/evaluator/cursors.rs:1555-1687`), and the sort comparators map errors to "equal", which Rust's sort may answer with a panic.

The maintainer decided that the engine becomes XPath 2.0 conformant in all of this, while PlatynUI.core's Locator keeps what its `index` and `position` mean to users.

## What Changes

- **Path results are in document order without duplicates** for every step shape and every model, with or without order keys.
- **Positional predicates count per context node.** `//X[1]` is the first X of every parent, and `(//X)[1]` is the first X overall. The optimizer no longer moves a possibly positional predicate from a parenthesized expression into a step.
- **The first match stays cheap.** Evaluation keeps streaming, and normalizes only where order cannot be proven. The usual locator shapes (`//T[p]`, `.//T[p]`, `A//T[p]`, `//A/T[p]`, chains of child steps) find their first result by reading no more of the tree than the nodes up to it and their ancestors.
- **`following::` keeps all results**, and context minimization is used only where every predicate of the step is non-positional.
- **Sorting is total.** The comparator-based repair cursor is replaced by a normalization cursor that orders by keys, or else by sibling-index paths built from the snapshot's cached lists. Set operations use the same order. Deduplication stays by identity, made cheap by an optional identity hint on nodes.
- **The Java agent scopes its runtime ids per view**, as UI Automation and JAB already do: a window listed flat under the desktop and the same window under its `app:Application` are two nodes with two ids. Desktop-wide `//Window` therefore keeps returning both copies, as it does for the other providers.
- **PlatynUI.core's Locator keeps its meaning.** When a locator on the `descendants` scope has an `index`, a `position` or raw predicates, it renders `descendant::X[…]` instead of `.//X[…]`, so `index` and `position` keep counting over all descendants.
- **Docs.** The engine's documentation (`dev-docs/architecture.md` §9.1–9.2, `crates/xpath/docs/xpath20_coverage.md`, the optimizer's and evaluator's doc comments) describes the new rules. BareMetal's "Finding elements" explains `(//X)[n]` against `//X[n]`.

Behavior changes that users see, for the release notes:

- `Query`, `platynui-cli query`, Inspector search and `Runtime.evaluate` list matches in document order, not grouped by tree level.
- A keyword that acts on one element, `Query only_first=${True}`, the waits, `Runtime.evaluate_single` and PlatynUI.core `find_one` use the first match in document order. When matches sit at different depths, that may be a different element than before.
- `//X[n]`, `A/X[n]`, `X[last()]` and `X[position()=n]` inside a step count per parent and may return several elements. `(//X)[n]` is the n-th match overall, which is what `//X[n]` returned before.
- Positional predicates on reverse axes count from each context node: `//B/ancestor::*[1]` is every B's parent.
- `following::` from nested context nodes no longer loses results.
- Paths whose order cannot be proven (sibling and reverse axes from several context nodes, filter-expression steps, paths from arbitrary sequences) collect their input before the first result.
- On the mock, `//control:Window[@Name=…]` first finds the copy under its `app:Application`, because the mock lists applications before its flat windows.
- Java agent elements get new runtime ids in their `app:Application` view (`agent/app/<pid>/<id>`).

## Capabilities

### New Capabilities

- `xpath-evaluation`: what the XPath engine returns for a path: document order without duplicates, positional predicates per context node, predicates on a parenthesized expression over the whole sequence, and that the first result does not require reading the rest of the tree.
- `core-locator`: what a PlatynUI.core Locator's `index` and `position` select, and how that is rendered.

### Modified Capabilities

- `baremetal-selector-resolution`: ADDED requirement that a selector matching several elements resolves to the first one in document order, and that positional predicates count per parent unless they are on a parenthesized selector.
- `java-provider`: ADDED requirement that the agent backend scopes runtime ids per view.

## Impact

- **Rust crates:**
  - `crates/xpath`:
    - the compiler (predicate classification, stream properties, where normalization is emitted);
    - the optimizer (pushdown only for non-positional predicates; `//T[p]` becomes `descendant::T[p]`);
    - the evaluator and cursors (per-context predicates, an order-preserving child-step merge, the normalization cursor, the `following::` fix, identity-based deduplication);
    - the model (an optional identity hint);
    - set operations;
    - tests and docs.
  - `crates/runtime`: `RuntimeXdmNode` implements the identity hint, and an attribute keeps a weak link to the element wrapper that listed it, so ordering attributes reads nothing again; new tests with the lazy fake of `tests/xdm_release.rs`.
  - `crates/provider-java`: per-view runtime ids and `SelectedItems` ids of the agent backend.
- **Python/RF:**
  - `src/PlatynUI/core/locator.py` and its tests;
  - BareMetal's library docs;
  - a new BareMetal mock suite for document order.
- **Tests:** table-driven tests with keyed and keyless models, a sweep over small trees against an independent naive evaluator, a guard on how much of the tree the first match reads, runtime, native and RF tests on the mock, and the acceptance lanes as a regression check.
- **Native rebuild:** yes.
- **Platforms:** every provider, because the engine is shared: UI Automation, AT-SPI, JAB, the Java agent and the mock. Only the Java agent's provider code changes.
- **Docs:** `dev-docs/architecture.md`, `crates/xpath/docs/xpath20_coverage.md`, `dev-docs/python-library-design.md` (the Locator), and the BareMetal library documentation.
- **Coordination:**
  - `xdm-snapshot-release` (implemented, not archived) owns `crates/runtime/src/xpath.rs` and its rule that a list of children is read at most once per query. The new sort must build its sibling indices from the snapshot's cached lists. The only changes to that file are the identity hint and the attribute's weak owner link, which keeps the snapshot free of strong cycles.
  - `application-process-attributes` and `snapshot-validity` touch the agent's `app.rs`, which the per-view ids also touch, so they land one after another.
