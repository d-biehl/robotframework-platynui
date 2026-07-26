## Context

Preparation refactor for the Java agent lane: `provider-java-swing` (and the `-javafx`/`-swt` follow-ups) become backend additions to one Java provider instead of competing providers. This change is deliberately **behavior-preserving** — its acceptance criterion is that the existing lanes pass unchanged. The driving insight: rank-based `window_claims` (agent > JAB > native) was pure coordination protocol between two Java claimants; with a single claimant the boolean registry suffices forever, because adding a backend changes which backend *serves* a claimed window, never who *claims* it.

## Goals / Non-Goals

**Goals:**

- Exactly one registered Java provider (`provider-java`), JAB as its first backend, behavior byte-for-byte compatible.
- Boolean `window_claims` preserved; no rank semantics anywhere.
- Java routing knowledge (classifier consumption) centralized in `provider-java`.
- Config namespace `providers.java.*` with per-backend subkeys.

**Non-Goals:**

- The agent itself and its injection/transport/delivery — that is `java-agent-core`; the backend that consumes it is `provider-java-swing`. Both are unchanged in substance by this refactor.
- Dynamic plugin loading of providers — future provider-plugin proposal (see [`dev-docs/provider-plugins.md`](../../../dev-docs/provider-plugins.md); the `platynui.providers` entry-point group from `java-agent-core` is its seed).
- Moving JVM **detection** out of the platform layer — see decision 4.
- Any change to JAB tree shape, roles, patterns, RuntimeIds, diagnostics, or timing.

## Decisions (proposed)

1. **Umbrella = router over a backend trait.** `provider-java` implements the UiTree-provider surface and delegates per top-level window to a backend. The trait mirrors what a provider already does per window (discover/claim candidates, serve subtree, patterns, degraded tracking) so the JAB code wraps without restructuring — the JAB crate (renamed `provider-java-jab`) keeps its internals (pump thread, handle hygiene, role mapping) and loses only its independent registration. `@Technology` stays backend-specific (`"JAB"`), so locators and the Inspector see no difference.

2. **Single claimant, boolean claims (the point of the whole change).** The claim rule is **"claim a window when one of the backends can serve it"**. Today the only backend is JAB, so the claimed set is exactly what the JAB provider claims now (`GetAccessibleContextFromHWND` success) and behavior is unchanged; UIA's `honor_window_claims` behavior is untouched. Stating the rule in its general form matters, because the backends do **not** all cover the same windows: JAB speaks `javax.accessibility`, which only Swing/AWT implements, so an SWT or JavaFX window has no JAB fallback at all — without an agent those windows are simply **not claimed** and stay with the native provider (UIA on Windows, AT-SPI on Linux). "What JAB can see" is therefore today's *extent* of the rule, not the rule itself.

    | Window | with an agent backend | without |
    |---|---|---|
    | Swing/AWT | agent backend | JAB backend (Windows) |
    | SWT | agent backend | **not claimed** → native provider |
    | JavaFX | agent backend | **not claimed** → native provider |

    When a backend with higher fidelity becomes available for an already-claimed window, the router switches the *serving* backend on the next enumeration pass — the claim itself never moves, so no registry protocol, no consumer updates, no re-routing races. When a backend becomes available for a window nobody claimed before (an agent appearing in an SWT JVM), the provider starts claiming it on that same pass, and the native provider yields as it already does for JAB.

3. **Config: `providers.java.enabled` (umbrella) + `providers.java.jab.enabled` / `providers.java.jab.call_timeout_ms` (backend).** Umbrella off ⇒ no Java provider at all (UIA shell serves Java windows, as with today's JAB kill switch). Backend off ⇒ that backend inert, others unaffected. The old `providers.jab.*` keys are renamed, not aliased (pre-1.0). `providers.windows-uia.honor_window_claims` is unaffected. The agent backend's keys (`providers.java.agent.*`) are reserved and land with the swing change.

4. **Detection stays platform-level; consumption centralizes (decided, documenting the boundary).** `java-app-classification` detection (`native:IsJvm`, `native:JvmToolkit`, reachability, the enablement diagnostic) remains a platform-bundle capability: it is the breadcrumb that tells a user *why* a window is empty and *that* a remedy exists — it must work precisely when no Java provider (or agent package) is active, and it is cheap (a module check; the agent-presence probe is a file-existence check that costs nothing when absent). What moves into `provider-java` is the *consumption*: which backend serves a JVM window, and any future auto-attach decision. This is the deliberate answer to "isn't the classifier too hard-wired?" — the wiring that stays pays for itself in diagnostics.

## Risks / Trade-offs

- [Refactor regression in JAB behavior] → the backend trait wraps rather than restructures; the Windows acceptance lane against the Swing fixture is the gate and must pass unchanged.
- [Config rename breaks existing setups] → accepted, and cheaper than assumed: pre-1.0 with no public release, so no setup exists that reads the old keys. The rename is loud in the changelog; the old section needs neither an alias nor a migration diagnostic, and the config layer ignores it like any other unclaimed section.
- [Trait shaped too narrowly for the agent backend] → the agent lane's design (multi-client RPC, degraded tracking mirroring JAB's, node validity) was written against the same provider surface; `java-agent-core`'s walking skeleton exercises it before the backend lands.

## Migration Plan

Internal refactor + config rename. Users: rename `providers.jab.*` keys to `providers.java.jab.*` (and use `providers.java.enabled` as the new master switch). Rollback: revert the change — no data or protocol surface involved.
