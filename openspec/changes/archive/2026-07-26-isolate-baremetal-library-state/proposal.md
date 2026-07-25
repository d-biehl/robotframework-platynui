# Isolate BareMetal Library State

## Why

`Set Root` stores its root in a single, globally named Robot Framework variable (`${PLATYNUI_ROOT_DESCRIPTOR}`, `src/PlatynUI/BareMetal/__init__.py:186-188`), and every `BareMetal` instance reads that same cell (`:849-860`). When a suite imports the library more than once — which the repo's own acceptance suites already do — one instance's root silently becomes every other instance's root. In `tests/acceptance/egui/coexisting_runtimes.robot` the suite setup runs `BM.Set Root … scope=SUITE` while `A` and `B` are separate instances with their own native runtimes; in `tests/acceptance/swing/dedup.robot` the same happens between `BM` and `BMOFF`. `${PLATYNUI_QUERY_SETTINGS}` (`:68-70`, `:862-877`) leaks identically, so one instance's `Set Query Settings scope=SUITE` retunes another instance's timeouts.

Isolating the variable is necessary but not sufficient, because a `UiNodeDescriptor` welds three things with different lifetimes into one object: a selector (pure data), a resolved node (bound to one `Runtime`), and its evaluation context (`library`, `overrides`). The context is captured at *creation* time and used at *resolution* time, so a descriptor read from a variable — or handed to another instance's keyword — resolves through the runtime and root of whoever created it. Two measured consequences (white-box probe with a stubbed runtime, `library.runtime` never touched by the resolving instance):

- A descriptor created by instance `A` and resolved while `B`'s keyword runs evaluates on `A`'s runtime against `A`'s root; `B`'s runtime sees no call at all.
- Within a *single* instance, the resolved node is cached on the shared per-query descriptor (`:129-131`, `:175`) keyed by the selector string alone — not by the root it was resolved against. `Set Root` to a different container followed by the same relative selector returns the **stale node from the previous root**, with no re-evaluation. The existing `baremetal-waiting` spec already works around this for `Wait Until Gone` alone ("SHALL NOT trust any node cached on the shared descriptor"); every other action and read keyword is exposed.

The Rust side is not implicated: the XDM tree cache is owned per runtime and keyed on the context node's `RuntimeId` (`crates/runtime/src/runtime/mod.rs:185`, `crates/runtime/src/xpath.rs:280-302`). The only process-global state is deliberate (`crates/core/src/platform/window_claims.rs:19`).

## What Changes

- **Per-instance scoped state.** `Set Root` and `Set Query Settings` store their value under a variable name derived from the name the calling instance is registered under in the Robot Framework namespace: the default import keeps `${PLATYNUI_ROOT_DESCRIPTOR}` / `${PLATYNUI_QUERY_SETTINGS}`, an aliased import gets a normalized `_<ALIAS>` suffix. Within a namespace that name maps one-to-one to an import, which is what makes it a complete discriminator.
- **The scope ladder is Robot Framework's, in full.** `scope` accepts the names RF's own `VAR` syntax accepts, with RF's meanings: `LOCAL`, `TEST` (alias `TASK`), `SUITE` (this suite — the boundary `Set Suite Variable` draws), `SUITES` (the suite *and* those below it, which makes a directory's `__init__.robot` usable as a fixture) and `GLOBAL`. Previously only three of them existed, with `SUITE` silently widened; now the default matches RF and the wider reach is something the caller names.
- **Only re-resolvable state may be given a cross-suite scope.** `SUITES` and `GLOBAL` hand the value to suites that build their own instance and runtime, in which a pinned element cannot be found again. Setting a root that pins an element (anywhere in its chain) at those scopes therefore fails on the spot and stores nothing, pointing at the selector form; at `LOCAL`/`TEST`/`SUITE` a pinned root stays allowed. Query settings are plain data and unrestricted.
- **Descriptors become pure selector data.** `library` and `overrides` move off `UiNodeDescriptor` and into the resolution call, so a descriptor is always resolved by the library whose keyword is running — against *its* root, *its* query settings and *its* runtime. Exchanging descriptors between instances becomes well-defined instead of silently wrong.
- **Implicit node caching is removed; capture becomes explicit.** A selector-backed descriptor SHALL be re-evaluated on every use. The two places that genuinely want a pinned element — `Wait Until Gone` with a captured element (`:1154-1180`) and `Set Root ${node}` — become explicit captures: runtime-bound, never re-resolved, and rejected with a clear error when a different instance tries to resolve them.
- **Nodes carry their runtime's identity.** `PyNode` holds only `Arc<dyn UiNode>` today (`packages/native/src/runtime.rs:32-35`), and the Python `UiNode` supports no weak references, so no pure-Python bookkeeping can tell a foreign node from a local one. Each `Runtime` gets a process-unique instance id, every node it produces carries it, and the binding exposes it — the additive native change that makes the guard above implementable for *any* node, including one the user holds in a variable.
- Docs: the library docstring's `Scoping queries to a container` section names `${PLATYNUI_ROOT_DESCRIPTOR}` (`:514-528`) and must describe the per-alias naming; `dev-docs/python-library-design.md` mentions both variables.

## Capabilities

### New Capabilities

- `baremetal-library-instances`: how several coexisting `BareMetal` imports keep their scoped state (root, query settings) apart, and how that state survives Robot Framework's per-suite re-instantiation.
- `baremetal-selector-resolution`: what a selector reference resolves against — always the calling library — and the split between a re-evaluated selector and an explicitly captured element.

### Modified Capabilities

- `baremetal-waiting`: the `Wait Until Gone` requirement is restated in terms of the selector/capture split; its "SHALL NOT trust any node cached on the shared descriptor" clause and the stale-cache scenario are subsumed by the general no-implicit-cache rule.

## Impact

- **Python/RF**: `src/PlatynUI/BareMetal/__init__.py` only — `UiNodeDescriptor` (`:99-183`), `descriptor_from_query` (`:800-807`), `root`/`query_settings` (`:849-877`), `set_root`/`set_query_settings` (`:879-1037`), and the keyword bodies that read `descriptor.node` (`:941-943`, `:1154-1180`, `:2128`) or pass a foreign node to their own runtime (`:1312`). `Query`'s `root: UiNode` argument (`:1043`) accepts a node from another instance's runtime and needs the same guard.
- **Rust**: additive, in the binding layer only — `packages/native/src/runtime.rs` (`PyRuntime` gains an instance id, `PyNode` an owner stamp propagated through the node-returning methods and the node iterators) plus `packages/native/python/platynui_native/_native.pyi`. No change to `crates/`. A native rebuild is required, and the mock-feature build (`just build-native-mock`) for the RF mock lane.
- **Tests**: new mock suites under `tests/BareMetal/` (multi-import isolation, stale-node re-evaluation, cross-instance descriptor rejection). `tests/acceptance/egui/coexisting_runtimes.robot` and `tests/acceptance/swing/dedup.robot` gain assertions that their roots stay separate — both are real-lane suites (X11 and Windows/JAB respectively). The repo's existing root tests use `Query`, which bypasses descriptors entirely, which is why the stale-node defect went unnoticed.
- **Behavior changes, and what they cost**: the scoped variable name changes for aliased imports; a stale-node "hit" that previously acted on the wrong element now re-resolves; passing a captured node between instances now fails loudly. None of this needs a compatibility shim or a deprecation path — the affected surface is this repo's own suites, so the work is adjusting tests where they encoded the old behavior. Nothing reads the variables directly today (only `dev-docs/python-library-design.md` names them).
- **Platforms/providers**: none specifically — this is library-layer behavior, exercised on the mock lane and confirmed on the X11 and Windows real lanes that already run multi-instance suites.
