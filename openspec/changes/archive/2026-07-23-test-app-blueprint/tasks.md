## 1. Blueprint capability

- [x] 1.1 Finalize the `test-app-blueprint` delta spec in this change (tiered catalog, canonical names, observables, CLI contract, shared-suite conventions, custom-controls chapter) and validate with `openspec validate --change test-app-blueprint`
- [x] 1.2 Cross-check the catalog against the pattern families in `dev-docs/python-library-design.md` §5a (read/action split, `@pattern_proxy_for` registry) so every family the default-proxy layer needs has a core- or extended-tier control, and note any deliberate gaps in the spec

## 2. Documentation alignment

- [x] 2.1 Rewrite `dev-docs/testing-strategy.md` §5 from the two-tier description (egui smoke / Qt native) to the technology-matrix framing: fixture apps per technology, blueprint conformance, shared catalog suite, per-technology lanes; write the blueprint itself into a new §5.1 (developer-facing description; the capability spec stays the acceptance-criteria form, kept in sync)
- [x] 2.2 Sweep fixture-app pointers (`AGENTS.md` orientation, fixture READMEs' cross-references) for statements contradicting the matrix framing; update only where wrong, no retrofit claims

## 3. Dependent-change alignment

- [x] 3.1 Verify the open `add-swt-test-app` and `add-javafx-test-app` changes reference the blueprint (core-tier adoption path, canonical names for new controls, catalog.robot onboarding) — done as part of this change set, kept consistent if the spec shifts during review
- [x] 3.2 Confirm `add-qml-test-app` (first full instance) matches the final blueprint wording before either change starts implementation

## 4. Verification

- [x] 4.1 `openspec validate --change test-app-blueprint` passes; `just check` stays green (docs-only change, no code impact)
- [x] 4.2 Follow-up retrofit changes for egui / Qt Widgets / Swing are captured as tracked future work (listed in the proposal; no implementation here)
