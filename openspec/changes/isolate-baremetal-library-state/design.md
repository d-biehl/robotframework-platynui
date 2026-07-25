# Design — Isolate BareMetal Library State

## Context

Three findings drive this change. The first two were measured, the third was read off the Rust source.

**1. RF re-instantiates the library per suite.** `BareMetal` is declared `scope='SUITE'` (`src/PlatynUI/BareMetal/__init__.py:238-244`). RF's `SuiteScopeManager.start_suite` sets `library.instance = None` (`robot/running/libraryscopes.py:90-103`), so every suite gets a fresh instance with a fresh `Runtime` (`:828-843`). Any identity derived from the instance object is therefore useless as a variable-name key: it changes at every suite boundary while the variable it names must not.

Correction found during implementation: this only bites in combination with D8. `set_root` calls `variables.set_suite(name, value)`, and RF's signature is `set_suite(name, value, top=False, children=False)` (`robot/variables/scopes.py:142`) — with the default, a suite-scoped value never reaches a child suite at all. Before D8, nothing crossed a suite boundary, every scoped value lived and died inside one instance's lifetime, and the registered name was a *complete* discriminator on its own (name ↔ instance is a bijection within a namespace: RF rejects a duplicate import of the same name). D8 makes suite values cross that boundary, which restores the case D3 guards.

**2. The registered library name is a stable, unique identity.** Measured against RF 7.4.2 with a throwaway `scope='SUITE'` library that resolves its own name from the namespace. Six scenarios, `id()` and the resolved name recorded:

| Scenario | instance | resolved name |
|---|---|---|
| parent suite, plain import | `#45056` | `['ProbeLib']` |
| child suite `a`, plain import | `#51632` | `['ProbeLib']` |
| child suite `a`, `AS SAME_ARGS` (identical args) | `#52016` | `['SAME_ARGS']` |
| child suite `a`, `AS OTHER` (different args) | `#48944` | `['OTHER']` |
| child suite `b`, plain import | `#97216` | `['ProbeLib']` |
| `Import Library … AS LATE` mid-test, after the plain import was used | `#44288` | `['LATE']` |

Three different instances of the same import resolve to the same name — exactly the stability an `id()`-based suffix lacks. No scenario produced two names for one instance, including the worst case (`AS LATE`), where `Importer.import_library` takes the cache hit and `TestLibrary.copy(name=alias)` copies the instance slot (`robot/running/importer.py:50-73`, `robot/running/testlibraries.py:258-269`) — the copy still got its own instance. An alias with *identical* import args also gets its own instance, so RF already separates everything a name would separate.

**3. The node cache is keyed by selector string alone.** Measured with a stubbed runtime (`evaluate_single` recorded, `UiNode` patched so `isinstance` passes):

```
same selector string, two different roots, ONE library
  resolve #1 -> <Node RT1:.//control:Text@ROOT_A>
  same descriptor object   : True
  resolve #2 -> <Node RT1:.//control:Text@ROOT_A>   <- ROOT_A, though the root is now ROOT_B
  evaluate_single calls    : [('.//control:Text', 'ROOT_A')]   <- one call, not two

descriptor created by A, resolved while B runs
  resolved by B -> <Node RT_A:.//control:Button@ROOT_OF_A>
  A.runtime calls          : [('.//control:Button', 'ROOT_OF_A')]
  B.runtime calls          : []
```

`__call__` consults `self.node` unconditionally (`:129-131`); the only invalidation is `is_valid()`, and a still-open window is valid. `descriptor_from_query` hands out one shared descriptor per query string (`:800-807`), so the cache is per (library, selector string) — with no notion of the root it was resolved against.

**Not implicated:** `XdmCache` is owned by the `Runtime` (`crates/runtime/src/runtime/mod.rs:185`) and its single slot is keyed on the context node's `RuntimeId` (`crates/runtime/src/xpath.rs:280-302`). Process-global state in the Rust core is limited to `window_claims::CLAIMS` (deliberate cross-runtime window claims, exercised by `dedup.robot`), a warn-dedup set, and the stateless XPath static context.

## Goals / Non-Goals

Goals:

- Coexisting `BareMetal` imports keep their scoped root and query settings apart, without losing parent→child suite inheritance.
- A selector reference always resolves against the library whose keyword is running.
- A selector is re-evaluated; only an explicit capture is pinned to a node.

Non-Goals:

- No change to the Rust core, the native bindings, or `Runtime` lifecycle (`per-runtime-platform-lifecycle` stands as is).
- No change to scope semantics (`LOCAL`/`TEST`/`SUITE`) or to how roots chain and drill.
- Not making runtimes share nodes: cross-runtime node exchange stays illegal, it just gets detected instead of silently mis-resolving.

## Decisions

**D1 — Variable suffix from the registered library name; default import keeps the bare name.** Rejected alternatives: `id(self)`/counter (breaks suite inheritance, see Context 1); a hash of the import args (an alias-only difference collides, and `${PLATYNUI_ROOT_DESCRIPTOR_A3F9C1}` is unreadable in the variable table); one variable holding `{library: descriptor}` (RF resolves scopes per variable — a `LOCAL` write by one instance would freeze and shadow another instance's `TEST` value inside the copied dict); keeping the state off RF variables and emulating scopes via listener callbacks (re-implements `LOCAL` keyword scoping for no gain and loses log visibility). Normalization: the name uppercased with every character outside `[A-Z0-9_]` replaced by `_`, prefixed with `_`; the default name maps to the empty suffix so the documented `${PLATYNUI_ROOT_DESCRIPTOR}` keeps working and dotted names never reach RF's extended variable syntax.

**D2 — Resolve the name via the namespace, reading `_instance`.** Iterate `EXECUTION_CONTEXTS.current.namespace._kw_store.libraries`, match `lib.code is type(self)` and `lib._instance is self`. The public `BuiltIn().get_library_instance(all=True)` route goes through `TestLibrary.instance`, a property that *creates* the instance when it is `None` (`robot/running/testlibraries.py:317-322`) — the probe confirmed the private read leaves other libraries uninstantiated (`Easter: False`) where the public one would not. Cache the result per instance; fall back to the empty suffix when there is no execution context.

**D3 — Import-args fingerprint travels in the value, not in the name.** With D8, D1's one remaining gap is the same registered name with different import args across a suite boundary (parent imports `use_mock=${True}`, child imports it with other arguments). Storing the fingerprint alongside the value keeps names readable and turns that case into a detected mismatch: ignore the inherited value and warn, rather than resolving through the wrong session. Verified on the mock lane — the child suite logs `Ignoring ${PLATYNUI_ROOT_DESCRIPTOR} set by a differently configured import` and falls back to the desktop.

**D4 — `library` and `overrides` become parameters of resolution, not fields of the descriptor.** They are properties of the *call*, not of the selector; capturing them at creation time is the direct cause of the cross-instance mis-resolution measured in Context 3. The existing `overrides` comment (`:123-126`) already documents a workaround for the same mistake — it must be written unconditionally on every call because the descriptor is shared.

**D5 — Remove implicit node caching; make capture explicit.** The code already needs the distinction twice and hand-rolls it both times: `set_root` deliberately refuses to copy the cached node into a query-backed root (`:941-943`, *"never copy the shared descriptor's cached node into it"*) and `Wait Until Gone` bypasses it for selectors (`:1142`) while depending on it for captured elements (`:1154-1180`). Making it explicit resolves the stale-node defect for every keyword at once. A capture carries its owning runtime and is rejected when another instance tries to resolve it; a selector never holds a node between calls.

**D6 — Guard `Query`'s `root: UiNode` argument (`:1043`) and `_maybe_bring_to_front` (`:1312`).** Both accept or forward a node that may belong to another instance's runtime. With D7's owner stamp the check is cheap and the failure becomes a clear error instead of an undefined evaluation.

**D7 — The owner stamp lives in the binding, not in Python.** Discovered while implementing D5/D6: a `UiNode` cannot be attributed to a runtime from Python. `PyNode` holds only `Arc<dyn UiNode>` (`packages/native/src/runtime.rs:32-35`); the Python `UiNode` supports no weak references (measured), so a Python-side registry would have to key on `id()` (recycled after GC) or pin every node it ever handed out; and `runtime_id` is *provider*-stable, not runtime-instance-scoped (`uia://desktop/2a.406ba`), so two runtimes on one session produce equal ids for the same element. Decision: `PyRuntime` takes a process-unique instance id from an `AtomicU64`, every `PyNode` it produces carries it, and both are exposed (`Runtime.instance_id`, `UiNode.owner_id`). Node-returning node methods (`parent`, `ancestors`, `children`, `top_level_or_self`) and the node iterators propagate the stamp of the node/runtime they came from.

**D8 — A suite-scoped value propagates to child suites (`children=True`).** Found while implementing: `scope=SUITE` stops at the suite that set it, because `set_suite` defaults to `children=False`. That makes a directory-level `__init__.robot` useless as a fixture — it can launch an application but cannot pin it as the root for the suites below, which is why each acceptance suite repeats the launch-and-pin flow. Passing `children=True` gives `SUITE` the meaning its name and documentation already imply ("every test in the suite", read naturally as the suite *tree*). This is a deliberate behavior extension beyond pure isolation; it is what makes D3 load-bearing rather than dead weight, and it is the one part of this change that can make an existing suite behave differently without being wrong before — a parent suite's root now reaches its children, where previously it did not.

Rejected alternative: wiring the existing `NodeResolver` (`crates/runtime/src/xpath.rs:65-102`, tested at `:1627-1723`) so a foreign node is *translated* into the local runtime's equivalent instead of rejected. Tempting because the machinery exists — `evaluate_options()` (`crates/runtime/src/runtime/evaluation.rs:15-17`) simply never sets a resolver, which is exactly why a foreign context node is used verbatim today. Rejected because `runtime_id` is only unique within a provider: for two runtimes bound to *different* sessions an id collision would silently retarget the query at the wrong element, trading a loud failure for a quiet one. Cross-runtime handles stay illegal; they just get detected.

**D9 — The root is resolved only when the selector needs it.** Found by the Windows acceptance lane, not by inspection. `tests/acceptance/swing/window.robot` closes the fixture process and then waits desktop-absolute for its window to vanish — deliberately, because "the suite root's app node dies with the process". It passed before this change only because the implicit cache still handed out an `is_valid()` node for the dead root; without the cache, resolving the root ran into its full timeout, for a query that never consults the context node. Every resolution now asks `runtime.is_context_dependent(query)` — already used by `set_root` (`:1000`) — and skips the root for an absolute selector. The result is cached per selector, since it is a function of the query string alone. The alternative, making a failing root resolution non-fatal, was rejected: it would silence a genuinely broken root instead of not asking a question that has no bearing on the answer. Bonus: absolute selectors save a root resolution per keyword (~0.7 ms, per the D5 measurements).

## Risks / Trade-offs

- **Losing the Python-side node cache costs an extra `evaluate_single` per keyword — measured at well under a millisecond.** Task 1.1, run against the installed real build (Windows UIA, live desktop, 30 repeats, first sample dropped):

  | | median | p90 |
  |---|---|---|
  | `evaluate_single`, warm `XdmCache` — what the Python cache saves | 0.72 ms | 0.79 ms |
  | `evaluate_single` after `clear_cache()` — what each retry attempt already pays | 3.93 ms | 4.67 ms |
  | root re-resolution, which every keyword performs regardless | 0.67 ms | 0.77 ms |
  | a realistic relative selector under a root | 1.15 ms | 1.67 ms |

  Removing the cache adds roughly one root re-resolution's worth of work to a keyword that already performs one, and is dwarfed by the action it precedes. Note also *when* the cache can help: the wait loops call `clear_cache()` between attempts (`:170`), so it never helps within a wait — only across separate keyword calls reusing the same selector string, which is exactly the situation that produces the stale node. Its only benefit occurs where it is wrong. D5 stands; the `(selector, root identity, runtime owner)` keying is not needed.
- **`_kw_store` and `_instance` are private RF API.** Both have been stable across RF 6/7; the lookup is one small helper with an empty-suffix fallback, so a future RF change degrades to today's behavior rather than crashing. Pinning the probe as a test (task 2.1) makes a break visible.
- **Existing suites may encode the old behavior.** A suite that unknowingly relied on a leaked root or on the pinned stale node changes behavior — that is the defect being fixed, but it can surface as a suite that starts failing, or one that only passed because two imports shared a root. No compatibility shim and no deprecation path are warranted: the affected surface is this repo's own suites, so the sweep in task 6.1 is the mitigation.

## Migration Plan

Behavioral, Python-only, no native rebuild required for the change itself (the mock lane needs `just build-native-mock` as usual). Roll-out is a single change to `src/PlatynUI/BareMetal/__init__.py` plus tests; roll-back is reverting that file. It lands in one step — no shim, no deprecation window, no dual-name fallback for the variables — because the only consumers are this repo's suites; where they encoded the old behavior they are adjusted in the same change.

## Open Questions

- Should a fingerprint mismatch (D3) warn-and-ignore, or fail the keyword? Ignoring matches RF's forgiving variable semantics; failing surfaces a genuine mis-configuration earlier.
- Should an explicit capture handed to another instance fail, or silently degrade to "not found"? Failing loudly is proposed; `Wait Until Gone` is the one keyword where "not resolvable" has a plausible reading as "gone", and it must not take it.
