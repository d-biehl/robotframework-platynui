## Why

The JAB provider (`add-jab-provider`) exposes Java Swing/AWT trees on Windows for top-down XPath queries, but not for **point-based hit-testing**: `UiTreeProvider::element_at_point` falls back to the default `UnsupportedOperation`. So the Inspector's live picker cannot select a Swing control by hovering it — it either resolves nothing or, via the UIA provider, the empty `SunAwtFrame` shell. Hit-testing is what makes the picker (and any "what is under the cursor?" feature) work against Swing apps.

There is also a latent cross-provider problem this surfaces: the runtime's `element_at_point` returns the **first** provider that yields a hit, in registration order, with no cross-provider z-order arbitration ([crates/runtime/src/runtime/input.rs](../../../crates/runtime/src/runtime/input.rs)). For a Java window both UIA (empty shell) and JAB (real subtree) could answer; without coordination the wrong one can win.

## What Changes

- `provider-jab` implements `element_at_point`: resolve the top-level window under the point (`WindowFromPoint` → root owner), gate on `isJavaWindow`, and for a Java window use the JDK's native hit-test `getAccessibleContextAt(vmID, rootContext, x, y)` to reach the deepest accessible context, then build a `JabNode` with the app-scoped `RuntimeId` and a walkable parent/ancestor chain up to `app:Application` (matching the ids top-down traversal produces) so the Inspector's reveal-and-select works. Non-Java points return `UnsupportedOperation` so other providers still handle them.
- `provider-windows-uia` abstains from claimed windows in `element_at_point`: when `providers.windows-uia.honor_window_claims` is on and the point's top-level window is claimed by another provider (the process-wide `window_claims` registry already used for root-streaming dedup), UIA returns `UnsupportedOperation` instead of the empty shell. This makes JAB win for Java windows **independent of provider registration order** and needs no runtime change.
- The JAB hit-test runs on the provider's pump thread under the same per-call deadline as every other JAB call, so an unresponsive JVM cannot hang the picker.
- Windows acceptance coverage: a picker/`element_at_point` scenario in `tests/acceptance/swing` resolving a known Swing control by its bounds-center point and asserting identity + single (non-duplicate) hit.

## Capabilities

### New Capabilities

- `jab-hit-test`: point-based hit-testing of Java Swing/AWT windows on Windows through the Java Access Bridge, including single-provider arbitration so a claimed Java window resolves to its JAB node rather than the UIA shell, and a reveal-ready ancestor chain for the Inspector picker.

### Modified Capabilities

<!-- none — jab-provider's spec did not cover element_at_point; the UIA abstain-on-claim behavior is specified here as part of jab-hit-test -->

## Impact

- **Modified crate**: `crates/provider-jab` — new `element_at_point` (bind `getAccessibleContextAt`; reuse the existing pump/handle/node machinery), plus a reveal ancestor-chain helper.
- **Modified crate**: `crates/provider-windows-uia` — `element_at_point` consults `window_claims` and abstains on claimed top-levels (config-gated, default on).
- **Native rebuild required**: `just build-native` for Python/Robot/Inspector consumers to get the picker behavior.
- **Tests**: new Windows real-provider picker scenario in `tests/acceptance/swing` (needs a JDK; skips otherwise); UIA config-gate unit test for the abstain path; mock lane unaffected.
- **Docs**: `dev-docs/platform-windows.md` (JAB hit-test + UIA claim-abstain note).
- **Depends on**: `add-jab-provider` (base provider, window-claims registry, scoped RuntimeIds). **Platform scope**: Windows only.
