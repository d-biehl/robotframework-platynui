## 1. Selection model in the registry

- [ ] 1.1 Resolve selection before factory construction: `providers.<id>.enabled` (default true) honored for every registered provider; a disabled provider is never constructed (no connection attempt, no library probing, no enablement diagnostics)
- [ ] 1.2 `providers.include` / `providers.exclude` as reserved non-id keys in the `providers` bucket (the `platform.backend` precedent); precedence per design 2 (explicit id flag → include → exclude → default on)
- [ ] 1.3 Fail-loud rule (design 3): an `include` matching no registered provider fails construction naming requested and registered ids; unknown entries alongside valid ones warn and are ignored; a deliberately empty set without `include` is allowed
- [ ] 1.4 Construction diagnostic listing active providers and the deciding rule per suppressed provider (design 6)

## 2. Robot library surface

- [ ] 2.1 Verify selection works through the **existing** `config=` import parameter with no library change (`config={'providers': {'include': [...]}}`) — this is the mechanism; everything below is convenience
- [ ] 2.2 *Optional* shorthand parameters (`providers=`, `exclude_providers=`) mapping to include/exclude; drop them if they do not clearly beat the nested dict
- [ ] 2.3 Acceptance for two selections in one suite, following the established patterns of `tests/BareMetal/library_instance_isolation.robot` and `tests/acceptance/egui/coexisting_runtimes.robot` (each aliased import is its own instance — the library is suite-scoped — so no import argument has to be varied to force that); verify the two instances keep separate scoped state under their per-name variables
- [ ] 2.4 Document in the library docs: selection via `config=`, that each aliased import is its own session, and why there is no live switching

## 3. Inspector

- [ ] 3.1 Provider toggles in the Inspector; toggling rebuilds the runtime and re-roots the tree (design 5), with the active-provider state visible
- [ ] 3.2 Suppressed-provider feedback in the UI (why a provider contributes nothing) consistent with the construction diagnostic

## 4. Verification

- [ ] 4.1 Mock-lane tests for the resolution matrix (all precedence combinations, fail-loud, portable cross-OS include lists)
- [ ] 4.2 Acceptance: a Java-only session shows no native-provider nodes for the fixture, and an unrelated non-Java suite is unaffected by the new keys
- [ ] 4.3 Absent selection keys reproduce current behavior exactly (regression guard); `just check`/`test`/`build-native` + relevant lanes green
