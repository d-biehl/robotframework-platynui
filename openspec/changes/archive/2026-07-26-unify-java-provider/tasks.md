<!-- Behavior-preserving refactor: the gate for every section is that the existing
     Windows acceptance lane (Swing fixture via JAB) and the mock lanes pass
     unchanged. Unblocks provider-java-swing (agent as second backend). -->

## 1. Umbrella crate + backend trait

- [x] 1.1 New crate `crates/provider-java` (name `platynui-provider-java`): UiTree-provider surface, per-window router, backend trait shaped after the existing per-window provider surface (discover/claim, serve subtree, patterns, degraded tracking, **node validity** — `UiNode::is_valid` is load-bearing for scoped-root reuse and must reach the backend, JAB answers it via `isSameObject` today)
- [x] 1.2 `provider-jab` becomes a library crate (renamed `crates/provider-java-jab`, package `platynui-provider-java-jab`): independent registration removed, internals (pump thread, handle hygiene, role mapping, diagnostics) untouched; wrapped as the first backend with `@Technology = "JAB"` preserved
- [x] 1.3 Runtime registration: exactly one Java provider registered on Windows builds; inert-absence behavior (no JAB DLL ⇒ no nodes, one diagnostic) preserved through the umbrella
- [x] 1.4 Claim rule implemented as **"a backend can serve it"**, not hard-wired to JAB's reach — with JAB as the only backend the claimed set is unchanged, but an SWT/JavaFX window stays unclaimed and with the native provider (design 2)
- [x] 1.5 Claim-owner id becomes the Java provider (`"java"` instead of `"jab"`) in `window_claims` and the UIA abstain check — a consequence of the single-claimant decision, not of the config rename; update the affected tests

## 2. Config namespace

- [x] 2.1 `providers.java.enabled` umbrella switch; rename all three JAB keys — `enabled`, `call_timeout_ms`, `dll_path` — to `providers.java.jab.*` (no aliases). ~~unknown old keys fail with a clear diagnostic~~ — dropped, see the note below
- [x] 2.2 Full reference sweep so no `providers.jab.*` mention survives: config lookups and `PROVIDER_ID` (`provider-java-jab/src/provider.rs`), the DLL-discovery **error message and the test asserting on it** (`dll.rs`), doc comments (`error.rs`, `lib.rs`), test configs (`tests/live_fixture.rs`), `dev-docs/platform-windows.md`, and the two main specs (`jab-provider`, `jab-hit-test`). Note: the literal `"jab"` also appears as the **claim-owner id** (`provider-windows-uia`, `window_claims` tests) — that one changes for a different reason (task 1.4), so do not blind-replace
- [x] 2.3 `providers.windows-uia.honor_window_claims` semantics unchanged; document the reserved `providers.java.agent.*` namespace

## 3. Classifier consumption

- [x] 3.1 Move `java-app-classification` consumption (backend routing for JVM windows) into `provider-java`; detection and the enablement diagnostic stay in the platform layer (design decision 4)

## 4. Verification & docs

- [x] 4.1 Windows acceptance lane (Swing fixture via JAB) green with **zero changes to the Robot suites** — that is the behavior-preservation proof; crate-level tests may only change mechanically (config keys per 2.2, claim-owner id per 1.4), never in their assertions about behavior. Mock lanes green; `just check`/`test`/`build-native` green
- [x] 4.2 Update docs/AGENTS pointers where providers are listed; changelog entry for the config rename

## Notes

- **Backend surface** (1.1): `JavaBackend` — `id`, `enumerate(parent) -> Enumeration { served_windows, nodes, unserved }`, `element_at_point`, `set_window_manager`, `shutdown`. Nodes travel unwrapped, which is what keeps `is_valid`, the pattern set and `@Technology` the backend's own answers; the router never proxies a node.
- **Where the claim rule lives** (1.4): a backend reports the windows it *can* serve; the router unions them into the claim set. `served_windows` is therefore the general rule, and "what JAB can see" is only today's extent of it.
- **What "consumption" concretely moved** (3.1): the enablement diagnostic. The JAB backend still *detects* Java-looking windows it cannot answer for (it needs `isJavaWindow` for that anyway) but now reports them as `unserved` instead of warning; the router filters them against the claim set and emits the shared `jvm_unreachable_diagnostic_once`. That is byte-for-byte today's behavior with one backend, and the rule generalizes correctly once a second one exists.
- **Legacy config** (2.1, changed during implementation): the migration diagnostic was built and then removed. The premise behind it — that someone could have `providers.jab.enabled = false` in a config and silently get the opposite after upgrading — does not hold: this is pre-1.0 with no public release, so no such config exists. Supporting the old spelling in any form, even as a warning, would be carrying weight for a user who does not exist. The old section is now ignored exactly like any other unclaimed one.
- **Scope of the routing implemented here** (1.4): the router claims the *union* of what the backends serve and concatenates their nodes. That is the whole claim rule and it is complete — one backend cannot overlap itself, so with JAB alone there is nothing to arbitrate. Selecting *between* backends for the same window (design decision 2's "switches the serving backend on the next enumeration pass") is not part of this change and needs a different `Enumeration` shape; it is `provider-java-swing` task 3.1, where the agent-presence signal that drives the selection actually exists. That task carries the constraint.
- **Why the backend stays its own crate** (1.2): not just size (4968 lines vs the router's ~600). JAB is Windows-only; the agent backend will not be. Keeping it separate lets the router's manifest become portable when `provider-java-swing` lands — `provider-java-jab` moves behind a `[target.'cfg(windows)'.dependencies]` edge and takes `libloading`, `sysinfo`, `chrono` and the `windows` features with it, instead of every one of them sitting in a manifest that has to build everywhere. The rename to `provider-java-jab` follows from the same shape: it names the family the next backends join.
- **Live fixture moved** (1.1/4.1): `tests/live_fixture.rs` moved from the JAB crate to `provider-java` — it drives the registered provider, which is now the umbrella. Assertions are unchanged; the nextest test-group filter and the acceptance recipe follow the new binary id.
- **Changelog** (4.2): `CHANGELOG.md` is generated by git-cliff from commit messages, so the entry is the commit body, not a hand-edit — `^refactor` maps to the "Refactor" group and is not skipped. No `BREAKING CHANGE:` footer: pre-1.0, so the rename is a normal change and the proposal's "**BREAKING**" marker is about the user-visible config surface, not about the commit convention.
