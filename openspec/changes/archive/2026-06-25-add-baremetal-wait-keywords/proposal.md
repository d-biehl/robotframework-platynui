## Why

Every BareMetal action and read keyword already waits for its target, and `Query` is the deliberate non-waiting snapshot for *asking about* the UI. But there is no keyword that **waits for an element and hands it back without acting**, none that **waits for an element to disappear**, and none that **waits until an arbitrary XPath result satisfies a condition**. Today users approximate these with `Sleep` or `Query` polling wrapped in `Wait Until Keyword Succeeds`, which is verbose, flaky, and inconsistent with the library's "state what you expect, the keyword waits for exactly that" model.

## What Changes

- Add **`Wait Until Exists`**: waits until a selector resolves to an element and returns that element. Element-only (a value/attribute selector fails loudly). Per-call timeout via `query_overrides` only.
- Add **`Wait Until Gone`**: waits until a selector matches nothing, or until a captured element becomes invalid; returns nothing. Rejects value-producing expressions (steers users to `Wait Until Query`). Captured-element invalidation depends on the provider's liveness check.
- Add **`Wait Until Query`**: waits until an XPath expression's result satisfies an AssertionEngine assertion, reusing the same `operator` / `expected` / `message` arguments as `Get Attribute`. Default (no operator) = wait until the result is truthy. It is **not** decorated with `@assertable`; it asserts inside its own retry loop.
- Add a new `BareMetalError` subclass **`ElementStillPresentError`** for `Wait Until Gone`'s "still present after timeout" failure (no existing class fits — `ElementNotFoundError` means the opposite).
- Give the native binding's evaluated-result types proper Python value semantics so query results test truthy and compare naturally — the foundation the truthy default builds on: **`__bool__`** on `UiNode` (→ `is_valid()`) and on `EvaluatedAttribute` (→ truthiness of its value), plus **`__eq__` / `__str__` / `__hash__`** on `EvaluatedAttribute` (delegating to its value). **BREAKING (behavioral, low-risk):** `bool(UiNode)` changes from always-`True` to `is_valid()`, and `EvaluatedAttribute == x` from always-`False` to value-based. No known consumer relies on the old behavior (all use `is_valid()` / `.value` explicitly); this is verified before merge.
- Document the three keywords as a new `== Waiting explicitly ==` subsection under the existing `= Waiting for elements =` section, and add the new error to the library's error taxonomy.

## Capabilities

### New Capabilities
- `baremetal-waiting`: explicit wait keywords for the BareMetal library (wait for an element to appear, to disappear, and for a query result to satisfy a condition), the dedicated still-present error, and the Python value semantics of evaluated query results that the truthy default relies on.

### Modified Capabilities
<!-- None: no existing specs under openspec/specs/; the query-settings and descriptor machinery is reused unchanged, not respecified. -->

## Impact

- **Affected specs:** `baremetal-waiting` (new).
- **Python:** `src/PlatynUI/BareMetal/__init__.py` — three new keywords, one new exception class, and docstring/section additions. Reuses the existing query-settings (`replace(self.query_settings, **query_overrides)`) and descriptor machinery unchanged; no changes to other keywords.
- **Rust / native binding:** `packages/native/src/runtime.rs` — dunder methods on `UiNode` and `EvaluatedAttribute`; `packages/native/python/platynui_native/_native.pyi` — stub updates. Requires a native rebuild (`just build`). `packages/native` lives outside the Cargo workspace.
- **Tests:** new mock suite `tests/BareMetal/wait_keywords.robot`; a Rust unit test for the dunders; an acceptance test under `tests/acceptance/egui/` for the captured-element "gone" success path (the mock never invalidates nodes, so that direction is only verifiable against a real provider).
- **Behavioral change** to the public native types' `bool()` / `==` — see *What Changes*.
