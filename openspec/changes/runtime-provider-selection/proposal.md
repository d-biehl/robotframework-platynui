## Why

Which providers a session uses is currently not selectable in any general way. Individual providers grew ad-hoc kill switches (`providers.jab.enabled`; `providers.java.enabled` arrives with `unify-java-provider`), but there is no convention, no way to say "this session is Java only", and no surface for it in the Robot library or the Inspector. That hurts concretely:

- Testing a Java app, the native provider (UIA/AT-SPI) still enumerates the whole desktop — slower queries and, for toolkits that render natively, competing representations to reason about.
- A suite cannot hold two views of the same desktop — e.g. one library instance restricted to Java and one to AT-SPI — which is the natural way to write tests that assert *which* technology serves what.
- Swapping a provider for a different one on the same platform (later: a WPF provider instead of UIA for WPF apps) has no expression at all.

The session-config foundation already exists (`runtime-session-config`: per-runtime, keyed by component id, fixed at construction). What is missing is a **selection model** on top of it plus the two user-facing surfaces.

## What Changes

- **Per-provider enablement as a convention**: `providers.<id>.enabled` (default true) is honored by the provider registry for *every* registered provider, instead of each provider implementing its own switch. A disabled provider is never constructed — no connection attempt, no probing, no diagnostics beyond one debug line.
- **Selection shorthand in the `providers` bucket**: `providers.include` (allowlist — only these run) and `providers.exclude` (blocklist), reserved non-id keys mirroring the existing `platform.backend` precedent. Precedence: `include` wins over `exclude`, and an explicit `providers.<id>.enabled = false` wins over both (explicit beats broad).
- **Fail loudly on an empty explicit selection**: if `include` names only ids no registered provider claims, construction fails naming the requested ids — mirroring the existing "requested platform backend cannot serve" rule, rather than silently yielding an empty desktop. A *typo alongside* valid entries is ignored with a warning.
- **Active-provider diagnostic**: at construction the runtime logs which providers are active and which were suppressed by which rule, so a mis-set filter is diagnosable instead of looking like a broken provider.
- **Robot library surface — mostly already there**: the library import already takes `config=`, so once the registry honors the keys, a suite can select providers with no library change at all:
  `Library PlatynUI.BareMetal config={'providers': {'include': ['java']}} AS JavaUI`. Two imports with different selections are two independent sessions — that foundation exists (`per-runtime-platform-lifecycle`, proven by `tests/acceptance/egui/coexisting_runtimes.robot`, with per-instance scoped state from `isolate-baremetal-library-state`). Optional sugar on top: `providers=java` / `exclude_providers=windows-uia` as shorthand for the nested dict — separable from the mechanism and droppable if it does not earn its keep.
- **Inspector toggles**: providers can be switched on/off in the Inspector; because session config is construction-fixed by design, a toggle **rebuilds the runtime** and re-roots the tree rather than mutating a live one.

## Capabilities

### New Capabilities

- `provider-selection`: the provider selection model (per-provider enablement, include/exclude with precedence, fail-loud on empty explicit selection, active-provider diagnostic) and its two surfaces — the Robot library import (multiple independent instances) and the Inspector toggles.

### Modified Capabilities

- `runtime-session-config`: the `providers` bucket gains the reserved non-id keys `include`/`exclude` (the `platform.backend` pattern applied to providers), and selection is resolved at construction like every other session setting — the immutability rule is unchanged.

## Impact

- **Modified**: provider registry/inventory resolution (gating before construction), session-config parsing (`providers.include`/`exclude`), Inspector (provider toggle UI + runtime rebuild), docs. The library import signature changes **only** if the optional shorthand is taken — the mechanism rides on the existing `config=` parameter.
- **No provider-internal changes**: gating happens above the providers; existing per-provider switches become instances of the convention (`providers.jab.enabled` is superseded by `providers.java.*` in `unify-java-provider` — no conflict, different level: umbrella/provider here, backend there).
- **Depends on `isolate-baremetal-library-state`**: without its per-instance scoped state (root, query settings keyed by registered library name) two imports with different selections would still share one root cell, so the multi-instance half of this feature would be broken in practice. Its write-time guard also already covers the risk this change creates — a scoped root that pins an *element* is refused at `SUITES`/`GLOBAL` (a suite below builds its own runtime and could not find that element again), so across a boundary a differently-selected session only ever sees selectors. Nothing to add here.
- **Composes with, and is independent of, the Java chain** — no ordering constraint; whichever lands second aligns naming. Also the natural home for later "WPF instead of UIA" selection, and unrelated to dynamic provider loading (that is the future provider-plugin proposal; selection needs no dynamic loading, only gating).
- No BREAKING changes: absent selection keys reproduce today's behavior exactly (all registered providers active).
