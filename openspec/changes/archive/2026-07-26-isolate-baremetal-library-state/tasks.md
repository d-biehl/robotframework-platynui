# Tasks — Isolate BareMetal Library State

## 1. Decide the node-cache question first

- [x] 1.1 Measure what the implicit node cache buys: time a mock-lane suite of repeated `Get Attribute` / `Pointer Click` calls on the same selector with the cache in place vs. with `descriptor.node` forced to `None` before each resolve. Record the numbers in design.md; if the cost is material, switch decision D5 to keying the cache on `(selector, resolved root identity, runtime owner)` and adjust the specs before continuing — measured against the installed real build (warm-cache resolve 0.72 ms median vs. 0.67 ms for the root re-resolution every keyword already performs); not material, D5 stands

## 2. Native owner stamp (before the Python work that depends on it)

- [x] 2.1 Binding test in `packages/native/tests/` (the crate has no Rust unit tests; its tests are pytest against the built module) pinning the stamp: two runtimes report different `instance_id`s, nodes report their producing runtime's id, and a node reached via `parent()`/`children()`/`ancestors()`/`top_level_or_self()` keeps it
- [x] 2.2 `PyRuntime` takes a process-unique instance id (`AtomicU64`), `PyNode` gains the owner field, all node-construction sites in `packages/native/src/runtime.rs` propagate it (node methods from the node, runtime methods from the runtime, iterators from whoever created them); expose `Runtime.instance_id` and `UiNode.owner_id`
- [x] 2.3 Update `packages/native/python/platynui_native/_native.pyi` and rebuild (`just build-native`) so the Python work below can use it

## 3. Robot Framework tests (mock lane, before the implementation)

- [x] 3.1 `tests/BareMetal/library_instances.robot`: two aliased imports; a `Set Root` through one leaves the other at the desktop, at `LOCAL`, `TEST` and `SUITE` scope; the same for `Set Query Settings`; assert the concrete variable names (`${PLATYNUI_ROOT_DESCRIPTOR}` for the unaliased import, `${PLATYNUI_ROOT_DESCRIPTOR_BM}` for `AS BM`)
- [x] 3.2 `tests/BareMetal/library_instance_inheritance/` suite with an `__init__.robot` that pins context twice, through two imports: at `SUITE` scope (plain import) and at `SUITES` scope (`AS TREE`). The child suites assert both ends of the boundary — `child_suite_scope.robot`: the `SUITE` root and settings do *not* reach the child, which starts at the desktop with its own defaults and can pin its own; `child_suites_scope.robot`: the `SUITES` root and settings *do* reach it and are re-resolved on the child's own runtime (`owner_id` comparison)
- [x] 3.3 `tests/BareMetal/scope_ladder.robot`: `TASK` behaves as `TEST`; a selector root and query settings can be set at `GLOBAL` (reset in the teardown, or it would rescope every later suite of the lane); a captured element is refused at `SUITES` and `GLOBAL` with the previous root left untouched, and still accepted at `SUITE`
- [x] 3.4 `tests/BareMetal/selector_resolution.robot`: the stale-node case — resolve a relative selector under root A, `Set Root` to B, resolve the same selector string, assert the element from B (mock tree: `//control:Window[@Name="Operations Console"]` and `//control:Window[@Name="Detail View"]` both contain a `control:Text`)
- [x] 3.5 Same file: a captured element passed to another import raises `BareMetalError`; `Query` with a foreign `root` node raises; `Wait Until Gone` with a foreign capture raises instead of reporting gone
- [x] 3.6 Python unit test pinning the name lookup (D2): a stub namespace exercising plain import, alias, and an instance registered under no name (empty-suffix fallback), plus an assertion that the lookup does not instantiate other libraries

## 4. Per-instance scoped state

- [x] 4.1 Add the name-resolution helper to `BareMetal` (`_kw_store.libraries`, match on `lib.code is type(self)` and `lib._instance is self`, cached per instance, empty-suffix fallback without an execution context)
- [x] 4.2 Route `set_root`, `set_query_settings`, the `root` property and the `query_settings` property through the derived variable names (`src/PlatynUI/BareMetal/__init__.py:849-1037`)
- [x] 4.3 Mirror RF's scope ladder (design D8): `scope` accepts `LOCAL`/`TEST`/`TASK`/`SUITE`/`SUITES`/`GLOBAL` and routes through the matching `VariableScopes` setter (`_write_scoped`), reading a scope back through `_scope_store`; `SUITE` keeps RF's default boundary. Unit-tested per scope name, including that only `SUITES` passes `children=True`
- [x] 4.4 Validate a pinned root once, on the chain, for every form the argument takes (fresh capture, restored root binding, selector root drilling into either): `_pinned_element` finds it, a foreign one raises `ForeignNodeError` at any scope, and an own one is refused at `SUITES`/`GLOBAL` (`UnsharableRootError`) — nothing is stored in either case. This is the write-time replacement for the two read-time guards that were built and removed here (the import-args fingerprint and `_usable_root`, design D3), and it closes the restore path, which previously bypassed `require_own_node` and only failed on the next lookup

## 5. Selector vs. capture

- [x] 5.1 Move `library` and `overrides` off `UiNodeDescriptor` into the resolution call (`:99-177`); update `descriptor_from_query` (`:800-807`), the argument converter (`:179-183`) and every keyword that resolves a descriptor
- [x] 5.2 Remove the implicit node cache (`:129-131`, `:175`) and introduce the explicit capture form, carrying its owning runtime's instance id
- [x] 5.5 Reuse the element a *scoped root* resolved to, gated on `is_valid()` and on the owner (design D5a): the one lookup per keyword the suite never wrote, and the one descriptor whose identity is its own invalidation. Target selectors stay uncached. Also: a dead capture now fails with `PinnedElementGoneError` naming the element, instead of falling through to "has no query to resolve the node" — the message matters now that capturing is the documented way to skip repeated lookups
- [x] 5.6 Document at `UiNode::is_valid` (`crates/core/src/ui/node.rs`) that the `true` default is for stubs only: with it, a scoped root is never resolved again, so any provider handing out nodes with a real lifetime owns the method. The mock does not override it, which is why the invalidation path is unit-tested (`tests/PlatynUI/test_baremetal_root_reuse.py`) and live-proven by `tests/acceptance/swing/window.robot`, not by a mock suite
- [x] 5.3 Rework the capture consumers: `set_root` with a node (`:941-943`), `Wait Until Gone`'s captured-element path (`:1154-1180`), `highlight` (`:2128`) — the two hand-rolled cache workarounds (`:941-943`, `:1142`) become unnecessary and are removed
- [x] 5.4 Guard foreign nodes: `Query`'s `root` argument (`:1043`) and `_maybe_bring_to_front` (`:1312`) raise `BareMetalError` on a node from another runtime

## 6. Docs

- [x] 6.1 Update the library docstring's `Scoping queries to a container` section (`:505-528`): per-import variable names, and that a root is private to its import
- [x] 6.2 Update `dev-docs/python-library-design.md` where it names `${PLATYNUI_ROOT_DESCRIPTOR}` / `${PLATYNUI_QUERY_SETTINGS}`
- [x] 6.3 Document the selector/capture distinction in the docstring where `Set Root` and `Wait Until Gone` describe node arguments

## 8. Coverage follow-up

- [x] 8.1 Cover the one scenario the mock tree cannot express — a selector whose matched element stays valid but stops satisfying it. `tests/acceptance/egui/interaction.robot`: the status label keeps its `@Id` and stays alive across a click while its `@Name` carries the count, so a selector keyed on the old name must stop matching. Asserts the element's liveness explicitly, without which the test would prove nothing
- [x] 8.2 Cover the legitimate counterpart of the foreign-element rejection — a *selector* handed to another import must resolve there, on that import's runtime (`tests/BareMetal/library_instance_isolation.robot`); it was the only requirement of `baremetal-selector-resolution` with no test at all

## 7. Existing suites and verification

- [x] 7.1 Sweep the existing suites for reliance on the old behavior — **no adjustment was needed**. Both multi-import suites already address the second import absolutely: `coexisting_runtimes.robot` uses `${WINDOW}` (= `/app:Application[@ProcessId=…]/(Frame|Window)`) and `dedup.robot` uses `/Window[…]`, so neither ever read the root it inherited from `BM`; the leak was latent, not load-bearing. No suite reuses one selector string under changing roots (the root suites go through `Query`, which never took the descriptor path). Nothing was silently repaired **in the multi-import suites** — but the acceptance lane later found what static inspection could not: `tests/acceptance/swing/window.robot` ("Close Ends The Fixture Process...") passed only because the stale cache served a dead root, and `tests/BareMetal/query_settings.robot` had two tests observing the root resolution through an *absolute* target, which no longer resolves the root at all (D9). Both were adjusted with the reason recorded in their documentation, and a test pinning the new behavior was added
- [x] 7.2 Add root-isolation assertions to `coexisting_runtimes.robot` (X11 lane) and `dedup.robot` (Windows/JAB lane) — both already import several instances and today share `BM`'s suite-scoped root
- [x] 7.3 Run `just check` and `just test-python`; run the mock lane (`just build-native-mock`, then `just test-baremetal`) for the new suites — `just check` clean, `just test` (Rust) 2055 passed / 3 skipped. Re-run after the D3/D8 rework: `just test-baremetal` 110/110, `just test-python` 782 passed, `just check` clean
- [x] 7.4 Run the Windows real lane — **96 tests, 95 passed, 1 failed**, re-run after each rework of the resolution path (D3/D8 scope ladder, then D5a root reuse) with the identical result. The one failure is the known order-sensitive `Acceptance.Egui.Inspector Toolbar.Completed Pick Announces The Picked Element`, verified as such after every run by executing it alone (passes; an Inspector input bug, not a query defect). Everything else green, including Swing/JAB 26/26 with `dedup.robot`'s isolation assertions and the three JAB live-fixture tests. Load-bearing for D5a: `Swing.Window.Close Ends The Fixture Process And Removes The Window` passes, i.e. a scoped root whose element dies with its process is resolved again — the invalidation path the mock lane cannot express. An earlier pre-rework run was 94/94
- [x] 7.5 Run the Linux real lanes on a Linux host — `just test-acceptance-x11` (the only lane that selects `coexisting_runtimes.robot`, i.e. the only live proof of the per-import root isolation and the foreign-element rejection: both new tests are `platform:x11`-tagged and never ran on Windows) and `just test-acceptance-compositor` (same library changes against the Wayland/AT-SPI paths). Reported green by the maintainer on a Linux host; the lane output was not captured in the session that wrote this note
