## Why

Java support is currently wired as independent pieces: the JAB provider registers as its own top-level provider, the planned agent provider (`provider-java-swing`) would register as a second one, and the consumption of `java-app-classification` sits inside the platform providers. Two independent Java claimants for the same windows would force `window_claims` from its boolean "claimed by other" check into **rank-based ownership** (agent > JAB > native) — new registry semantics plus updates to every generic consumer — purely as a coordination protocol between two halves of what is conceptually one thing. Consolidating first removes that cost before it is ever paid: **one Java provider**, boolean claims exactly as today, and the agent lands later as an internal backend. It also pulls Java routing knowledge out of places that don't need it, addressing the creeping hard-wiring.

## What Changes

- **New crate `crates/provider-java`** (crate name `platynui-provider-java`): the single registered Java provider — a thin router over toolkit **backends** behind a backend trait. `provider-jab` becomes a library crate wrapped as the first backend; all JAB behavior (tree, roles, patterns, pump thread, diagnostics, `@Technology = "JAB"`) is **unchanged**.
- **Claims stay boolean**: `provider-java` is the sole Java claimant; the UIA provider's `honor_window_claims` logic is untouched. No rank machinery, now or later — a second backend changes which backend serves a claimed window, never who claims it.
- **Classifier consumption centralizes**: routing decisions that consume `java-app-classification` move into `provider-java`. Detection itself (`native:IsJvm` etc. + the enablement diagnostic) deliberately **stays in the platform layer** — it is the breadcrumb that tells a user *why* a Java window is empty and *that* a remedy exists, and must therefore work when no Java provider is active.
- **Config namespace migration**: all three JAB keys (`enabled`, `call_timeout_ms`, `dll_path`) move from `providers.jab.*` to `providers.java.jab.*`, plus a new umbrella switch `providers.java.enabled`; room is reserved for `providers.java.agent.*` (the agent backend's keys land with `provider-java-swing`). The rename includes the user-visible DLL-discovery error message.
- **Pure refactor otherwise**: no behavior change — the existing Windows acceptance lane (Swing fixture via JAB) and the mock lanes must pass unchanged; that green run *is* the verification.

## Capabilities

### New Capabilities

- `java-provider`: the single Java UiTree provider routing per-window to toolkit backends, with umbrella and per-backend enablement and the single-appearance guarantee.

### Modified Capabilities

- `jab-provider`: registration now happens via the `java-provider` umbrella (JAB as a backend, no longer a top-level provider) and the config keys move to `providers.java.jab.*`; all tree/pattern/robustness behavior is unchanged.
- `jab-hit-test`: the deadline key it references moves to `providers.java.jab.call_timeout_ms`; hit-test behavior is unchanged.

## Impact

- **New**: `crates/provider-java`. **Modified**: `crates/provider-jab` (becomes a library backend; registration removed), runtime provider registration, config handling (**BREAKING** config-key rename `providers.jab.*` → `providers.java.jab.*` — pre-1.0, no compatibility aliases), docs/AGENTS pointers where providers are listed.
- **No behavior change** to trees, locators, claims, or diagnostics; `just build-native` required as usual.
- **Depends on**: nothing open (`java-app-classifier` is archived/landed). **Unblocks**: `provider-java-swing`, which then adds the agent as a second backend instead of a second provider and drops its rank-based-claims design entirely; likewise simplifies the `provider-java-swt`/`-javafx` follow-ups.
