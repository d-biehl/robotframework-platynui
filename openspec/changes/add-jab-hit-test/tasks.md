## 1. JAB client: native hit-test

- [ ] 1.1 `ffi.rs`/`dll.rs`: bind `getAccessibleContextAt` (`unsafe extern "C" fn(VmId, JObject64, i32, i32, *mut JObject64) -> BOOL`); add a `JabClient::context_at(window_ctx, x, y)` returning `Option<JabObject>` on the pump thread with the per-call deadline (RAII release like the other handle-returning calls)

## 2. Provider: element_at_point

- [ ] 2.1 `provider.rs`: implement `UiTreeProvider::element_at_point` — `WindowFromPoint(point)` → `GetAncestor(GA_ROOT)`; skip the host process's own window; `isJavaWindow` gate (else `Err(UnsupportedOperation)`); `getAccessibleContextFromHWND` + `context_at` for the deepest context
- [ ] 2.2 Reveal chain: build the picked `JabNode` scoped `IdScope::App { pid }` with a strong parent chain up to `app:Application`, mapping the hit context to its enumeration-index path via a single bounded top-down re-walk of the owning window matched with `isSameObject` (so the RuntimeId equals top-down traversal); documented fallback to a window-scoped parentless node when matching fails
- [ ] 2.3 Return `Ok(Some(node))` for a Java hit, `Ok(None)` when the point is over a Java window but no context resolves, `Err(UnsupportedOperation)` for non-Java/own-process points

## 3. UIA: abstain on claimed windows

- [ ] 3.1 `provider-windows-uia::element_at_point`: when `providers.windows-uia.honor_window_claims` is true, resolve the point's top-level window and return `Err(UnsupportedOperation)` if it is claimed by another provider in `platynui_core::platform::window_claims` — before calling `ElementFromPoint`; unit-test the config gate (claimed → abstain; kill switch off → resolves)

## 4. Acceptance & verification

- [ ] 4.1 `tests/acceptance/swing`: picker scenario — resolve the stage-1 button by its bounds-center point (`BM.Element At Point` or the equivalent), assert `@Name`/`@Technology="JAB"` and a single (non-duplicate) hit; assert the picked node's RuntimeId equals the top-down-located one and its `app:Application` ancestor resolves
- [ ] 4.2 Robustness: extend the frozen-JVM lane (or the Rust live-fixture lane) with a hit-test-during-freeze assertion (bounded return, UIA elsewhere responsive)
- [ ] 4.3 `dev-docs/platform-windows.md`: JAB hit-test + UIA claim-abstain note; then `just check`, `just test`, `just build-native`, and the Windows acceptance run green
