## Context

`runtime-session-config` already establishes what this builds on: config is per-runtime, grouped into `platform`/`providers`, keyed by component id, and **fixed at construction**. This change adds the selection layer and the two surfaces users asked for (Robot library import, Inspector). It deliberately does **not** touch how providers work internally — selection is gating *above* them.

Two existing facts shape the design:

- **`platform.backend` is the precedent** for a reserved non-id key inside a bucket, so `providers.include`/`providers.exclude` follow an established pattern rather than inventing one.
- **Construction-fixed config is a feature, not an obstacle.** "Switch a provider off at runtime" is therefore expressed as "build a runtime with a different selection" — for a suite that means a second library instance, for the Inspector a rebuild. This keeps the invariant that a runtime's view of the world never changes underneath a running query.

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

4. **The library surface takes a list, not a boolean matrix.** `providers=java` / `providers=java,atspi` maps to `include`; `exclude_providers=` maps to `exclude`. Each library instance builds its own runtime, so two instances are two independent sessions — which is exactly the semantic the suite author wants when comparing technologies. Robot's `WITH NAME` gives them distinct keyword namespaces; nothing is shared between them.

5. **Inspector: toggle = rebuild.** The toggle edits the pending selection and rebuilds the runtime, re-rooting the tree (the same path an Inspector already takes for a fresh session). Rejected: making provider sets live-mutable — it would break the construction-fixed invariant for every consumer to serve one interactive convenience.

6. **Diagnostic names the deciding rule.** At construction, one log line lists active providers and, per suppressed provider, *why* (explicit flag / not in include / in exclude / not registered on this platform). Without this, a filtered-out provider is indistinguishable from a broken one — the most likely support question this feature creates.

## Risks / Trade-offs

- [Users expect live switching] → documented as "build another runtime"; the Inspector shows the intended UX (toggle rebuilds), and per-suite the two-instance pattern is the answer.
- [Two instances double resource use] → intended and bounded; each session is as heavy as a normal one, and the pattern is opt-in.
- [Typo in an include list yields a surprising session] → mitigated by decision 3 (fail loud when nothing matches) plus decision 6 (the diagnostic names the rule).
- [Overlap with the per-backend switches inside the Java provider] → different levels: this change gates *providers*; `providers.java.jab.enabled` selects a *backend inside* the Java provider. Documented explicitly to avoid a config muddle.

## Migration Plan

Purely additive: no selection keys ⇒ today's behavior (all registered providers active). Existing `providers.<id>.enabled` switches keep working and become instances of the convention. Rollback: revert; no persisted state involved.
