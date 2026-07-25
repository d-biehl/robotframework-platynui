## Context

`runtime-session-config` already establishes what this builds on: config is per-runtime, grouped into `platform`/`providers`, keyed by component id, and **fixed at construction**. This change adds the selection layer and the two surfaces users asked for (Robot library import, Inspector). It deliberately does **not** touch how providers work internally — selection is gating *above* them.

Four existing facts shape the design — most of the multi-instance story is already built:

- **`platform.backend` is the precedent** for a reserved non-id key inside a bucket, so `providers.include`/`providers.exclude` follow an established pattern rather than inventing one.
- **Construction-fixed config is a feature, not an obstacle.** "Switch a provider off at runtime" is therefore expressed as "build a runtime with a different selection" — for a suite that means a second library instance, for the Inspector a rebuild. This keeps the invariant that a runtime's view of the world never changes underneath a running query.
- **The library import already takes `config=`.** Selection therefore needs **no library-surface change at all** to be usable: the moment the registry honors the keys, `config={'providers': {'include': ['java']}}` works. This shrinks the change to its actual core (the registry) and makes the convenience parameters genuinely optional.
- **Coexisting sessions are proven, not aspirational.** `per-runtime-platform-lifecycle` ("runtimes share no platform state") plus `tests/acceptance/egui/coexisting_runtimes.robot` establish that two imports are two independent native runtimes; `isolate-baremetal-library-state` makes their *scoped* state (root, query settings) per-instance as well. This change adds the one missing ingredient — different provider sets per session — rather than the coexistence machinery.

## Goals / Non-Goals

**Goals:**

- One convention that gates every provider, replacing ad-hoc per-provider switches.
- Two independent library instances with different provider sets in one suite.
- Inspector toggles without breaking config immutability.
- Mis-selection is loud, not silently empty.

**Non-Goals:**

- Dynamic loading/unloading of provider code (DLLs, plugins) — that is the future provider-plugin proposal. Selection only gates what is already linked in.
- Live mutation of a running runtime's provider set.
- Per-query or per-locator provider scoping — selection is a session property. (Locator-level technology filtering already exists via `@Technology`.)
- Replacing a provider's own connection settings — those stay in its sub-dictionary.

## Decisions (proposed)

1. **Gate before construction, in the registry.** The registry resolves the selection and only then constructs the surviving factories. Consequence: a disabled provider does no connection attempt, no library/DLL probing, and emits no enablement diagnostics — the property the Java lane calls "quiescence" (`provider-java-swing`), here generalized to every provider.

2. **Precedence: explicit beats broad.** Resolution order per provider id: `providers.<id>.enabled` if present → else `include` if present (member ⇒ on, non-member ⇒ off) → else `exclude` (member ⇒ off) → else on. Rationale: the narrow, per-id statement is the one the author wrote knowingly; the lists are the convenience layer. `include` and `exclude` may be combined, and `include` then bounds the set that `exclude` trims.

3. **Empty explicit selection fails, empty implicit selection does not.** If `include` is present and no registered provider matches any entry, construction fails naming the requested ids and the registered ones — the same "don't silently degrade" stance as the existing `platform.backend` rule. Unknown entries *next to* valid ones are ignored with a warning (portable configs across OSes must stay usable: `include: [uia, atspi]` is legitimate on both Windows and Linux). Ending up with zero providers *without* an explicit `include` (e.g. everything excluded deliberately) is allowed — that is a valid, if empty, session.

4. **The mechanism is the registry; the library parameters are optional sugar.** Selection is fully usable through the existing `config=` import parameter (`config={'providers': {'include': ['java']}}`), so this change does not *need* a new library surface. If the shorthand is taken — `providers=java,atspi` → `include`, `exclude_providers=` → `exclude` — it is a flat convenience for the nested dict and can be dropped without touching the mechanism. Two consequences worth stating explicitly, because both are easy to get wrong:

    - **Each aliased import is its own instance** — the library is `@library(scope='SUITE')`, and for a suite- (or test-) scoped library Robot Framework instantiates per import entry, so two aliases are two instances even with identical arguments (measured; only a `GLOBAL`-scoped library would share one instance via the import cache). Two selections are therefore always two sessions, and no argument needs to be varied artificially to force that.
    - **Cross-session leakage is already handled, and needs nothing from this change** (`isolate-baremetal-library-state`): scoped values live under a name derived from the *registered library name*, so two differently-aliased instances never share a cell; and a scoped root that pins an *element* is guarded on **write** — refused at `SUITES`/`GLOBAL`, because a suite below builds its own runtime and could not resolve that element again. A reader across a boundary therefore only ever sees selectors, whatever its own selection is. (An earlier read-time import-argument fingerprint was built and removed there; do not reintroduce a variant of it here.)

5. **Inspector: toggle = rebuild.** The toggle edits the pending selection and rebuilds the runtime, re-rooting the tree (the same path an Inspector already takes for a fresh session). Rejected: making provider sets live-mutable — it would break the construction-fixed invariant for every consumer to serve one interactive convenience.

6. **Diagnostic names the deciding rule.** At construction, one log line lists active providers and, per suppressed provider, *why* (explicit flag / not in include / in exclude / not registered on this platform). Without this, a filtered-out provider is indistinguishable from a broken one — the most likely support question this feature creates.

## Risks / Trade-offs

- [Users expect live switching] → documented as "build another runtime"; the Inspector shows the intended UX (toggle rebuilds), and per-suite the two-instance pattern is the answer.
- [Two instances double resource use] → intended and bounded; each session is as heavy as a normal one, and the pattern is opt-in.
- [Typo in an include list yields a surprising session] → mitigated by decision 3 (fail loud when nothing matches) plus decision 6 (the diagnostic names the rule).
- [Overlap with the per-backend switches inside the Java provider] → different levels: this change gates *providers*; `providers.java.jab.enabled` selects a *backend inside* the Java provider. Documented explicitly to avoid a config muddle.

## Migration Plan

Purely additive: no selection keys ⇒ today's behavior (all registered providers active). Existing `providers.<id>.enabled` switches keep working and become instances of the convention. Requires `isolate-baremetal-library-state` for the multi-instance half (per-instance scoped state); the registry half stands alone. Rollback: revert; no persisted state involved.
