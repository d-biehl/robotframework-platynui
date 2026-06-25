## 1. Native binding — value semantics

- [x] 1.1 Add `__bool__` to `UiNode` in `packages/native/src/runtime.rs`, returning `self.is_valid()`.
- [x] 1.2 Add `__bool__` to `EvaluatedAttribute` returning the truthiness of its value, plus `__eq__` (delegate to `.value`), `__str__` (`str(value)`), and `__hash__` (delegate to `.value`), mirroring the existing `core.rs` dunder pattern.
- [x] 1.3 Update `packages/native/python/platynui_native/_native.pyi` stubs for the new dunders on `UiNode` and `EvaluatedAttribute`.
- [x] 1.4 Add pytest tests for the new dunders in `packages/native/tests/` (UiNode truthiness reflects validity; EvaluatedAttribute bool/eq/ne/str/hash reflect the value, including a falsy `False` via `@IsMaximized`). Note: `packages/native` builds with `extension-module`/`abi3` (no `auto-initialize`), so pyo3 classes are tested through pytest, not Rust `#[test]`.
- [x] 1.5 Rebuild the native extension (`just build`) and confirm no existing consumer relies on the old `bool()`/`==` behavior (grep `is_valid()` / `.value` usages across `src/`, `apps/`, `packages/`).

## 2. Error type

- [x] 2.1 Add `ElementStillPresentError(BareMetalError)` after `InvalidSelectorError` in `src/PlatynUI/BareMetal/__init__.py`, with a docstring matching the existing four error classes.

## 3. Shared mechanics

- [x] 3.1 Add a private `_assertion_value(result)` helper that unwraps an `EvaluatedAttribute` to its `.value` and passes `UiNode`/native values through, for use as the `verify_assertion` input.
- [x] 3.2 Confirm the settings merge `replace(self.query_settings, **(query_overrides or {}))` and a `clear_cache()`-per-attempt / `time.monotonic()` loop shape that mirrors `UiNodeDescriptor.__call__`.

## 4. Wait Until Exists

- [x] 4.1 Implement `wait_until_exists(self, descriptor, *, query_overrides=None) -> UiNode` decorated with `@keyword`.
- [x] 4.2 Assign `descriptor.overrides = query_overrides` unconditionally, delegate to `descriptor()`, and on `ElementNotFoundError` re-raise with the user-facing message `No element matched {query!r} within timeout of {timeout} seconds.` (timeout from the merged settings); let `ResultTypeError` propagate.
- [x] 4.3 Write the `doc_format='ROBOT'` docstring (Brief / detail / Args / Returns / Examples), documenting the element-only contract and the captured-node validation case.

## 5. Wait Until Gone

- [x] 5.1 Implement `wait_until_gone(self, descriptor, *, query_overrides=None) -> None` decorated with `@keyword`.
- [x] 5.2 CASE A (selector, `descriptor.query is not None`): per attempt `clear_cache()` then `evaluate_single(descriptor.query, self.root)`; first-attempt type guard raising `ResultTypeError` (pointing to `Wait Until Query`) for a non-`None`, non-`UiNode` result; "gone" ⇔ `result is None`; never trust `descriptor.node`.
- [x] 5.3 CASE B (captured node, `descriptor.query is None`): per attempt `clear_cache()` then "gone" ⇔ `not descriptor.node.is_valid()`.
- [x] 5.4 Use an explicit per-attempt `gone` flag; a swallowed exception under `ignore_exceptions` sets `gone = False`. Re-resolve `self.root` per attempt (no snapshot). Raise `ElementStillPresentError` (selector / captured-node message) on timeout.
- [x] 5.5 Write the docstring, documenting both target shapes, the value-expression rejection, the provider-dependent captured-node invalidation, and the missing-root behavior.

## 6. Wait Until Query

- [x] 6.1 Implement `wait_until_query(self, expression, assertion_operator=None, assertion_expected=None, assertion_message=None, *, root=None, query_overrides=None) -> Any` with `@keyword`, the three assertion params declared directly right after `expression` (so RF binds `expr == ${x}` positionally like `Get Attribute`; `root` is keyword-only) and no `@assertable`; import `AssertionOperator, verify_assertion` from `assertionengine`.
- [x] 6.2 Reject the `then`/`evaluate` operator before the loop with a clear error pointing to `validate`/comparison operators.
- [x] 6.3 Resolve `ctx = root if root is not None else self.root` once. Loop: `clear_cache()`, `evaluate_single(expression, ctx)`.
- [x] 6.4 No-operator branch: success = `bool(result)` (relies on the native dunders); never call `verify_assertion`. With operator: call `verify_assertion(_assertion_value(result), op, expected, message or "")`, success on no exception.
- [x] 6.5 Broaden the per-attempt catch: re-raise `RuntimeError` and `(SystemExit, KeyboardInterrupt)`; treat `AssertionError`/`TypeError` (and, under `ignore_exceptions`, evaluation errors) as not-satisfied-this-attempt.
- [x] 6.6 On success return the raw `result` (no operator) or `verify_assertion`'s return (with operator).
- [x] 6.7 On timeout: default path raises a "did not become truthy … within timeout of {timeout} seconds." error; assertion path performs a final `verify_assertion` outside the loop and re-raises `AssertionError` with the timeout context appended.
- [x] 6.8 Write the docstring, reusing the operator-list phrasing and AssertionEngine link, documenting the truthiness default, that it operates on raw XPath results (distinct from `Get Attribute`), and that `then`/`evaluate` are unsupported.

## 7. Documentation

- [x] 7.1 Add a `== Waiting explicitly ==` subsection under `= Waiting for elements =` (after `== Tuning the wait ==`) introducing the three keywords with backtick cross-links (`Wait Until Exists`, `Wait Until Gone`, `Wait Until Query`, `Get Attribute`, `Query`, `Set Query Settings`, `Set Root`).
- [x] 7.2 Add `ElementStillPresentError` to the library's error taxonomy near the other error classes.

## 8. Mock test suite (RF, fast, deterministic)

- [x] 8.1 Create `tests/BareMetal/wait_keywords.robot` modeled on `query_settings.robot` (`use_mock=${True}`, `query_settings={'timeout': 0.2}`, `${OPS}`, `${MISSING}`).
- [x] 8.2 Wait Until Exists: returns element; missing → user-facing timeout error; `query_overrides` timeout; `Set Query Settings` scope; non-element → `ResultTypeError`; no-leak across the shared cache.
- [x] 8.3 Wait Until Gone: already-gone selector returns fast; persisting selector → `ElementStillPresentError`; captured still-valid → times out; stale-cache ignored; value selector → `ResultTypeError`; `query_overrides` timeout; `ignore_exceptions` + malformed selector must time out (never falsely gone).
- [x] 8.4 Wait Until Query: default native truthy/falsy; falsy-attribute default times out (dunder guard); default returns the raw result; `==` pass and timeout-with-diagnostic; operator-None + truthy-expected raises no `ValueError`; order/regex operator survives pre-appearance attempts; `then`/`evaluate` rejected; `validate` polls; `query_overrides` timeout; `root=` parameter; `.../@X` vs `Get Attribute` parity; `ignore_exceptions` interaction.

## 9. Acceptance tests (egui lane, real provider)

- [x] 9.1 Create an egui acceptance suite (e.g. `tests/acceptance/egui/wait.robot`) following the lane conventions: a PID-pinned app instance per suite, role + `@Id` selectors anchored to the app, real `PlatynUI.BareMetal` (no mock).
- [x] 9.2 Wait Until Exists end-to-end: trigger a widget/panel to appear (e.g. open a dialog/window), wait for it, and assert the returned element (role / `@Id`).
- [x] 9.3 Wait Until Gone (selector direction): close a window/dialog and wait until its selector matches nothing.
- [x] 9.4 Wait Until Gone (captured-node direction): capture an element via `Query`, destroy/close it, and assert `Wait Until Gone ${el}` returns — the path the mock cannot cover (it never invalidates nodes); also keep the still-valid negative direction.
- [x] 9.5 Wait Until Query end-to-end: assert a real attribute / `count()` condition that becomes true after an action (e.g. wait until a list has ≥ n items, or `//Button[@Id="…"]/@IsEnabled == ${True}` after enabling it).

## 10. Verification

- [x] 10.1 `just check` (fmt, clippy, ruff, mypy) and `just test-python` pass; run the new Rust dunder test.
- [x] 10.2 Run the new mock suite and confirm `robotcode` reports it green.
- [x] 10.3 Run the egui acceptance suite in the real compositor/X session and confirm green. (Compositor 4/4, X11 4/4, headless.)
- [x] 10.4 `openspec validate add-baremetal-wait-keywords` (if available) passes.
