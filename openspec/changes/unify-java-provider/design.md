## Context

Preparation refactor for the Java agent lane: `provider-java-swing` (and the `-javafx`/`-swt` follow-ups) become backend additions to one Java provider instead of competing providers. This change is deliberately **behavior-preserving** — its acceptance criterion is that the existing lanes pass unchanged. The driving insight: rank-based `window_claims` (agent > JAB > native) was pure coordination protocol between two Java claimants; with a single claimant the boolean registry suffices forever, because adding a backend changes which backend *serves* a claimed window, never who *claims* it.

## Goals / Non-Goals

**Goals:**

- Exactly one registered Java provider (`provider-java`), JAB as its first backend, behavior byte-for-byte compatible.
- Boolean `window_claims` preserved; no rank semantics anywhere.
- Java routing knowledge (classifier consumption) centralized in `provider-java`.
- Config namespace `providers.java.*` with per-backend subkeys.

**Non-Goals:**

- The agent backend — that is `provider-java-swing`, unchanged in substance.
- Dynamic plugin loading of providers — future provider-plugin proposal (the `platynui.providers` entry-point group from the swing change is its seed).
- Moving JVM **detection** out of the platform layer — see decision 4.
- Any change to JAB tree shape, roles, patterns, RuntimeIds, diagnostics, or timing.

## Decisions (proposed)

1. **Umbrella = router over a backend trait.** `provider-java` implements the UiTree-provider surface and delegates per top-level window to a backend. The trait mirrors what a provider already does per window (discover/claim candidates, serve subtree, patterns, degraded tracking) so the JAB code wraps without restructuring — `provider-jab` keeps its internals (pump thread, handle hygiene, role mapping) and loses only its independent registration. `@Technology` stays backend-specific (`"JAB"`), so locators and the Inspector see no difference.

2. **Single claimant, boolean claims (the point of the whole change).** `provider-java` claims Java windows exactly as the JAB provider does today (`GetAccessibleContextFromHWND` success); UIA's `honor_window_claims` behavior is untouched. When a future backend with higher fidelity is available for a claimed window, the router switches the *serving* backend on the next enumeration pass — the claim itself never moves, so no registry protocol, no consumer updates, no re-routing races.

3. **Config: `providers.java.enabled` (umbrella) + `providers.java.jab.enabled` / `providers.java.jab.call_timeout_ms` (backend).** Umbrella off ⇒ no Java provider at all (UIA shell serves Java windows, as with today's JAB kill switch). Backend off ⇒ that backend inert, others unaffected. The old `providers.jab.*` keys are renamed, not aliased (pre-1.0). `providers.windows-uia.honor_window_claims` is unaffected. The agent backend's keys (`providers.java.agent.*`) are reserved and land with the swing change.

4. **Detection stays platform-level; consumption centralizes (decided, documenting the boundary).** `java-app-classification` detection (`native:IsJvm`, `native:JvmToolkit`, reachability, the enablement diagnostic) remains a platform-bundle capability: it is the breadcrumb that tells a user *why* a window is empty and *that* a remedy exists — it must work precisely when no Java provider (or agent package) is active, and it is cheap (a module check; the agent-presence probe is a file-existence check that costs nothing when absent). What moves into `provider-java` is the *consumption*: which backend serves a JVM window, and any future auto-attach decision. This is the deliberate answer to "isn't the classifier too hard-wired?" — the wiring that stays pays for itself in diagnostics.

## Risks / Trade-offs

- [Refactor regression in JAB behavior] → the backend trait wraps rather than restructures; the Windows acceptance lane against the Swing fixture is the gate and must pass unchanged.
- [Config rename breaks existing setups] → accepted (alpha); the rename is loud in the changelog and the old keys fail with a clear unknown-key diagnostic rather than being silently ignored.
- [Trait shaped too narrowly for the agent backend] → the swing change's design (multi-client RPC, degraded tracking mirroring JAB's) was written against the same provider surface; the spike there validates the fit before the agent backend lands.

## Migration Plan

Internal refactor + config rename. Users: rename `providers.jab.*` keys to `providers.java.jab.*` (and use `providers.java.enabled` as the new master switch). Rollback: revert the change — no data or protocol surface involved.
