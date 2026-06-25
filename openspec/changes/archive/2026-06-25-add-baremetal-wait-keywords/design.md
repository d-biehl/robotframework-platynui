## Context

BareMetal (`src/PlatynUI/BareMetal/__init__.py`) already waits implicitly: every action/read keyword resolves its target through `UiNodeDescriptor.__call__` (`:123`), a retry loop that polls `runtime.evaluate_single` every `retry_interval`, clears the runtime cache between attempts, breaks on the first non-`None` `UiNode`, and raises `ElementNotFoundError` ("…within timeout of {timeout} seconds.") on timeout. `Query` (`:927`) is the deliberate non-waiting snapshot. Waiting is configured by `QuerySettings` (`timeout`, `retry_interval`, `ignore_exceptions`) layered import-default → `Set Query Settings` scope → per-call `query_overrides`.

Three new keywords add the *explicit* counterpart: wait for appearance and return the element, wait for disappearance, and wait until an XPath result satisfies a condition. The technical assumptions below were verified against the real code (pyo3 bindings in `packages/native/src/runtime.rs`, the installed `assertionengine`, the mock provider) before this design was fixed.

Key verified facts that shape the design:

- `evaluate_single` returns `UiNode | EvaluatedAttribute | UiValue | None`. `UiValue` is a **type alias** for native Python types — `count()` / `string()` already return native values; an empty node-set returns `None`; `.../@X` returns an `EvaluatedAttribute`; `//X` returns a `UiNode`.
- `EvaluatedAttribute` and `UiNode` have **no `__bool__`** in the current binding, so `bool(attr)` / `bool(node)` are always `True`; `EvaluatedAttribute` has no `__eq__`, so `attr == x` is always `False`. The dunder pattern (`__eq__`/`__hash__`/`__str__`/`__len__`) is already used in `packages/native/src/core.rs` for Point/Size/Rect.
- `UiNode.is_valid()` (`runtime.rs:177`) is provider-dependent. The core trait default returns `true` and the **mock never overrides it**, so a captured node's `is_valid()` is always `True` against the mock — the captured-element "gone" *success* path is only verifiable against a real provider.
- `verify_assertion(value, operator, expected, message="")` returns the value (or a transformed value for `then`/`matches`) on success and raises `AssertionError` on a mismatch — but it also raises `ValueError` when `operator is None` and `expected` is truthy, `TypeError` from order/regex operators on incomparable or non-string operands (exactly the early, pre-appearance attempts where the value is `None`), and `RuntimeError` for an unknown operator. `then`/`evaluate` call `BuiltIn().evaluate(...)` and never raise on a "mismatch"; `validate` raises `AssertionError` on a falsy expression.
- `@assertable` (`_assertable.py`) calls `verify_assertion` exactly once, so it cannot be reused for a polling keyword.
- `Get Attribute` returns `node.attribute()` — a native value, never an `EvaluatedAttribute` — so `Wait Until Query .../@X` (which gets an `EvaluatedAttribute` from `evaluate_single`) is a *different code path* with different missing-attribute behavior (`None` vs `AttributeNotFoundError`).

## Goals / Non-Goals

**Goals:**
- Add `Wait Until Exists`, `Wait Until Gone`, `Wait Until Query` as explicit synchronization keywords, consistent with the library's existing wait machinery and configured solely through `query_overrides` for per-call timeouts.
- Make evaluated query results test truthy and compare naturally, fixing a footgun for all binding consumers, not just these keywords.
- Reuse the existing query-settings and descriptor machinery without changing it.

**Non-Goals:**
- No new per-call `timeout=`/`retry_interval=` arguments (per-call tuning stays on `query_overrides`).
- No change to the implicit wait of existing action/read keywords, nor to `Query`'s non-waiting semantics.
- No attempt to make `EvaluatedAttribute` a fully transparent proxy for its value (regex/`contains`/ordering still operate on the unwrapped value); only `__bool__`/`__eq__`/`__str__`/`__hash__` are added.

## Decisions

**D1 — Fix truthiness/equality in the Rust binding, not with a Python helper.** Add `__bool__` to `UiNode` (→ `is_valid()`) and `EvaluatedAttribute` (→ truthiness of `.value`), plus `__eq__`/`__str__`/`__hash__` to `EvaluatedAttribute` (delegating to `.value`; `__hash__` is required because defining `__eq__` otherwise makes the type unhashable). The truthy default of `Wait Until Query` then becomes a plain `bool(result)`. *Alternative considered:* a Python-only `_result_is_truthy` helper that special-cases the wrapper types — rejected because it leaves the footgun in place for every other consumer (`IF ${result}`, the legacy `PlatynUI` core, the Inspector).

**D2 — `Wait Until Query` asserts in its own loop, not via `@assertable`.** The decorator asserts once; a waiting assertion must re-evaluate. The three assertion parameters are declared on the signature directly (same names/spellings as `Get Attribute`), and `verify_assertion` is called per attempt.

**D3 — Unwrap `.value` for the assertion input.** Even with the new dunders, `contains` (`in`), `matches` (`re.search`), and order operators require the native value, not the wrapper. So `verify_assertion` is always given the meaningful value (`EvaluatedAttribute.value`; node/native pass through). The justification is "`evaluate_single` returns a wrapper", **not** "match Get Attribute" — those are different code paths.

**D4 — Reject `then`/`evaluate`; support `validate`.** `then`/`evaluate` never raise on a mismatch, so they cannot express a wait (they would "succeed" on the first attempt). They are rejected up front with a clear error pointing to `validate`. `validate` raises `AssertionError` on a falsy expression and therefore polls correctly.

**D5 — Broaden the caught exceptions in the assertion loop.** Catch `AssertionError` *and* `TypeError` (and, under `ignore_exceptions`, evaluation errors) as "not satisfied this attempt"; let `RuntimeError` (unknown operator — a programming error) and `SystemExit`/`KeyboardInterrupt` propagate. This prevents the wait from dying on the first attempt when order/regex operators hit `None`/typed early values. On timeout, a final `verify_assertion` outside the loop surfaces the engine's real diagnostic.

**D6 — Return contract mirrors the existing two keywords.** Without an operator, `Wait Until Query` returns the raw evaluation result (like `Query`: `EvaluatedAttribute` — now value-like and carrying `.owner()` — `UiNode`, or native). With an operator, it returns `verify_assertion`'s return value (like `Get Attribute`'s `@assertable`). The slight asymmetry (raw vs unwrapped for attributes) is exactly the existing `Query` vs `Get Attribute` convention.

**D7 — `Wait Until Gone` has two paths and never trusts a cached node.** Selector target: re-evaluate `runtime.evaluate_single(query, self.root)` fresh each attempt (with `clear_cache()`); "gone" ⇔ result is `None`. A non-`None`, non-`UiNode` result (a value expression like `count()`) raises `ResultTypeError` on the first attempt, pointing to `Wait Until Query`, rather than waiting to a confusing timeout. Captured-node target (descriptor with `query is None`): "gone" ⇔ `not node.is_valid()`. A swallowed exception (`ignore_exceptions`) maps to an explicit `gone = False`, never to the `None` that means success. The root is re-resolved per attempt, matching the library's tested "root re-resolves on every lookup" contract (a vanished `Set Root` root surfaces as the root's `ElementNotFoundError`).

**D8 — `Wait Until Exists` delegates but normalizes the message.** It assigns `descriptor.overrides = query_overrides` unconditionally (so a `None` clears any prior override on the shared cached descriptor, preserving the no-leak invariant) and calls `descriptor()` to reuse the wait loop and node cache. It catches `ElementNotFoundError` and re-raises with user-facing wording ("No element matched …") while keeping the `within timeout of {timeout} seconds.` glob; `ResultTypeError` propagates verbatim (element-only contract).

**D9 — New `ElementStillPresentError(BareMetalError)`.** `Wait Until Gone`'s timeout failure has no existing class (`ElementNotFoundError` means the opposite). A focused subclass is added next to the other four error classes, with a message ending in the standard glob suffix.

**D10 — Per-call timeout via `query_overrides` only.** The hand-written loops merge settings as `replace(self.query_settings, **(query_overrides or {}))`, reusing the exact scoped/default base and the per-call override path; no new arguments are introduced.

## Risks / Trade-offs

- `bool(UiNode)` now triggers a provider liveness check (I/O) → acceptable; that is the intended semantics, and it is documented. A tight `if node:` loop pays for a backend call, which is the same cost the wait keywords need anyway.
- Behavioral change to public types' `bool()`/`==` → grep shows no consumer relies on the old always-`True`/always-`False` behavior; this is verified systematically before merge, and called out as BREAKING (behavioral) in the proposal.
- Captured-element "gone" *success* is not testable against the mock (it never invalidates) → covered by an acceptance test in the egui lane; the mock covers the still-valid (negative) direction.
- `Wait Until Query` operates on raw XPath results and diverges from `Get Attribute` on *missing* attributes (`None` vs `AttributeNotFoundError`) → documented, with a parity test for the present-attribute case so the divergence is caught if it widens.
- Even with the new dunders, `contains`/`matches`/ordering need the native value → always unwrap `.value` for `verify_assertion` (D3).

## Open Questions

- Exact pyo3 slot for `__bool__` is confirmed at implementation time (the `__eq__`/`__hash__`/`__str__` pattern already exists in `core.rs`; `__bool__` is the only new slot). No blocking unknowns.

## Migration Plan

- The native change is additive at the API level but behavioral for `bool()`/`==`; it requires a native rebuild (`just build`). No data migration. Rollback is a straightforward revert of the binding and Python changes; the dunders and keywords are independent of stored state.
