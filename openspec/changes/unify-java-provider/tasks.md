<!-- Behavior-preserving refactor: the gate for every section is that the existing
     Windows acceptance lane (Swing fixture via JAB) and the mock lanes pass
     unchanged. Unblocks provider-java-swing (agent as second backend). -->

## 1. Umbrella crate + backend trait

- [ ] 1.1 New crate `crates/provider-java` (name `platynui-provider-java`): UiTree-provider surface, per-window router, backend trait shaped after the existing per-window provider surface (discover/claim, serve subtree, patterns, degraded tracking, **node validity** — `UiNode::is_valid` is load-bearing for scoped-root reuse and must reach the backend, JAB answers it via `isSameObject` today)
- [ ] 1.2 `provider-jab` becomes a library crate: independent registration removed, internals (pump thread, handle hygiene, role mapping, diagnostics) untouched; wrapped as the first backend with `@Technology = "JAB"` preserved
- [ ] 1.3 Runtime registration: exactly one Java provider registered on Windows builds; inert-absence behavior (no JAB DLL ⇒ no nodes, one diagnostic) preserved through the umbrella
- [ ] 1.4 Claim rule implemented as **"a backend can serve it"**, not hard-wired to JAB's reach — with JAB as the only backend the claimed set is unchanged, but an SWT/JavaFX window stays unclaimed and with the native provider (design 2)
- [ ] 1.5 Claim-owner id becomes the Java provider (`"java"` instead of `"jab"`) in `window_claims` and the UIA abstain check — a consequence of the single-claimant decision, not of the config rename; update the affected tests

## 2. Config namespace

- [ ] 2.1 `providers.java.enabled` umbrella switch; rename all three JAB keys — `enabled`, `call_timeout_ms`, `dll_path` — to `providers.java.jab.*` (no aliases); unknown old keys fail with a clear diagnostic
- [ ] 2.2 Full reference sweep so no `providers.jab.*` mention survives: config lookups and `PROVIDER_ID` (`provider-jab/src/provider.rs`), the DLL-discovery **error message and the test asserting on it** (`dll.rs`), doc comments (`error.rs`, `lib.rs`), test configs (`tests/live_fixture.rs`), `dev-docs/platform-windows.md`, and the two main specs (`jab-provider`, `jab-hit-test`). Note: the literal `"jab"` also appears as the **claim-owner id** (`provider-windows-uia`, `window_claims` tests) — that one changes for a different reason (task 1.4), so do not blind-replace
- [ ] 2.3 `providers.windows-uia.honor_window_claims` semantics unchanged; document the reserved `providers.java.agent.*` namespace

## 3. Classifier consumption

- [ ] 3.1 Move `java-app-classification` consumption (backend routing for JVM windows) into `provider-java`; detection and the enablement diagnostic stay in the platform layer (design decision 4)

## 4. Verification & docs

- [ ] 4.1 Windows acceptance lane (Swing fixture via JAB) green with **zero changes to the Robot suites** — that is the behavior-preservation proof; crate-level tests may only change mechanically (config keys per 2.2, claim-owner id per 1.4), never in their assertions about behavior. Mock lanes green; `just check`/`test`/`build-native` green
- [ ] 4.2 Update docs/AGENTS pointers where providers are listed; changelog entry for the config rename
