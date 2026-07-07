## 1. Pointer Scroll keyword

- [x] 1.1 Add a `ScrollDirection` value type (`Literal["UP","DOWN","LEFT","RIGHT"]`, or a small enum + converter) used by the keyword; pick the lightest option that gives good libdoc and a clear error on an invalid value. → chose the `Literal` (matches the existing `scope: Literal[...]` house style; RF validates it and gives a clear invalid-value error). Default is the string `'DOWN'`.
- [x] 1.2 Implement `pointer_scroll(self, descriptor=None, *, direction='DOWN', ticks=1, x=None, y=None, overrides=None, activate=None, query_overrides=None) -> None` with `@keyword`.
- [x] 1.3 Map `direction`+`ticks` to the native `(h, v)` delta (one notch = 120): `DOWN`→`(0, -120*ticks)`, `UP`→`(0, +120*ticks)`, `LEFT`→`(+120*ticks, 0)`, `RIGHT`→`(-120*ticks, 0)`. The keyword owns the sign so the requested direction is the visible one. Horizontal sign set from the X11 wheel-button convention (button 6 = left for +h, button 7 = right for −h), giving one consistent rule — a *negative* component increases the offset on each axis, so both `DOWN` and `RIGHT` are negative. (This corrects the original draft, which had `RIGHT` positive; the egui acceptance suite asserts the visible direction.)
- [x] 1.4 Body: assign `descriptor.overrides = query_overrides`; `_maybe_bring_to_front(descriptor, activate)`; resolve the point with `_resolve_screen_point`; when a point is resolved, `runtime.pointer_move_to(point, overrides)` first; then `runtime.pointer_scroll((h, v), overrides)`. With no target, scroll at the current position (no move).
- [x] 1.5 Write the `doc_format='ROBOT'` docstring (Brief / detail / Args / Returns / Examples), documenting the notch unit (1 tick = 120 units), move-over-target behavior, and `${None}` for the current position.

## 2. Documentation

- [x] 2.1 Add the Robot Framework scroll usage to `dev-docs/pointer-input.md` (it already documents CLI/Python scroll and claims RF consistency — close that divergence).
- [x] 2.2 Cross-link `Pointer Scroll` from the BareMetal pointer/`Input timing and motion` docs where the other pointer keywords are introduced. → added a `== Pointer scrolling ==` subsection under `= Input timing and motion =` documenting the `scroll_step`/`scroll_delay_ms` profile fields; `Pointer Scroll` auto-links in libdoc.

## 3. egui scroll-box fixture

- [x] 3.1 Add a dedicated, fixed-size scroll box to `apps/test-app-egui` (a `ScrollArea` whose content always overflows), built from non-scroll-consuming label rows, each with a stable `@Id`. → `show_scroll_box`: a `ScrollArea::both()` capped at 220×120 with 40 wide, wrap-disabled label rows (`@Id` `scrollbox-row-NN`), placed in a dedicated right-hand `SidePanel` (outside the central `ScrollArea`, so over-scrolling clamps at the box edges instead of leaking to an outer area).
- [x] 3.2 Render the scroll area's current offset (`ScrollArea::show(...).state.offset`) into a label with a stable `@Id` (e.g. `scrollbox-offset`) placed outside the box so it stays put and readable. → two labels below the box, `scrollbox-offset-x` / `scrollbox-offset-y`, whose text is the bare integer offset (so a test can assert `number(@Name)`).
- [x] 3.3 Make the box scroll both axes (or add a horizontal companion) so one fixture covers `UP`/`DOWN`/`LEFT`/`RIGHT`. → `ScrollArea::both()` with content overflowing on both axes.

## 4. Tests

- [x] 4.1 pytest (fake runtime): assert the `direction`+`ticks` → `ScrollDeltaLike` mapping for each direction, the default (`DOWN`, 1 tick → `(0, -120)`), and that a descriptor target triggers a move-to-point before the scroll. → `tests/PlatynUI/test_baremetal_pointer_scroll.py` (9 tests, green); also covers overrides forwarding and the invalid-direction error.
- [x] 4.2 RF mock suite `tests/BareMetal/pointer_scroll.robot` (`use_mock=${True}`): missing target → not-found error honoring the timeout and `query_overrides`; scroll over an element then `Get Pointer Position` equals the element's point; `${None}` scrolls at the current position without moving. → 5 tests, green via `just test-baremetal` (82/82).
- [x] 4.3 egui acceptance suite `tests/acceptance/egui/scroll.robot`: hover the scroll box, `Pointer Scroll DOWN`, and assert via `Wait Until Query` that `scrollbox-offset` y increases; `Pointer Scroll UP` restores it to ~0; repeat for `RIGHT`/`LEFT` on x. Assert direction + reversibility, not exact pixels. → scrolls at a fixed point captured inside the box (`Reset The Box` over-scrolls back to 0 for order-independence); asserts `number(.../@Name) > 0` then `== 0`. Green on both backends (see 5.3).

## 5. Verification

- [x] 5.1 `just check` (fmt, clippy, ruff, mypy) and `just test-python` pass. → `just check` green (clippy workspace `-D warnings`, ruff, mypy 124 files); the new pytest (9) and mock suite (5) are part of the Python/mock lanes.
- [x] 5.2 Run `just test-baremetal` and confirm the new mock suite is green. → 82/82, including the 5 `Pointer Scroll` tests.
- [x] 5.3 Run the egui acceptance lane (`just headless=true test-acceptance-compositor` and `-x11`) and confirm the scroll suite is green on both backends. → **both green, both directions**: Wayland compositor 2/2, X11 2/2 (verified headless and visible). This is what exposed the platform-layer scroll bugs in §6.
- [x] 5.4 `openspec validate add-baremetal-pointer-scroll` passes.
- [x] 5.5 Windows acceptance (`just test-acceptance-windows`, UIA provider on a real desktop): confirm the scroll suite is green — the `platform-windows` HWHEEL sign fix (§6.3) is reasoned + cross-compiled, but not runtime-verified here (no Windows host). → verified on Windows by the user: scroll works in all directions (incl. horizontal HWHEEL sign).

## 6. Platform scroll-injection fixes (exposed by the first real scroll keyword)

The keyword is a thin wrapper, but it is the *first* code path to actually emit scroll through the real
platforms — which surfaced latent, never-exercised bugs in the platform layer. Making scroll observably
work end-to-end (the acceptance suite) required these; they are part of this change.

- [x] 6.1 **Runtime chunking** (`crates/runtime/src/pointer.rs`): `component_steps` chunked an axis whose
  `scroll_step` component is 0 into **1-unit micro-steps**. Since `scroll_step` defaults to `(0, -120)`
  (horizontal 0), every horizontal scroll became hundreds of sub-notch events — negligible on Wayland and
  rounded to *zero* wheel clicks on X11, so horizontal never moved. Fixed to fall back to one wheel notch
  (120) per step. Affects all platforms; fixes horizontal everywhere. (Rust unit tests green.)
- [x] 6.2 **Wayland compositor** (`apps/wayland-compositor/src/control.rs` `inject_pointer_scroll`): injected
  scroll as `AxisSource::Finger` with only a continuous value; switched to `AxisSource::Wheel` with the
  discrete `v120` amount (+ pixel value), like real wheel input (`input::handle_pointer_axis`). Also negates
  the delta to translate PlatynUI's convention (down/right negative) to Wayland's (down/right positive).
- [x] 6.3 **Windows provider** (`crates/platform-windows/src/pointer.rs`): Win32 `MOUSEEVENTF_HWHEEL` is
  positive = right — the opposite of PlatynUI's `RIGHT = −h` (and of X11 button 7). Negate the horizontal
  delta for `HWHEEL` so `Pointer Scroll RIGHT` scrolls right on Windows; vertical (`MOUSEEVENTF_WHEEL`)
  already matches. Reasoned + cross-compiled (`x86_64-pc-windows-gnu`); runtime verification pending 5.5.

## 7. Follow-ups (from implementation review)

- [x] 7.1 Rust regression test for the chunking fix (6.1) in `crates/runtime/src/pointer.rs`:
  `component_steps`/`scroll_steps` chunk a zero-`scroll_step` axis by one notch (120), not 1-unit
  micro-steps; plus an engine-level test that a 3-notch horizontal scroll emits 3 notch-sized events, not
  360. Green via nextest.
- [x] 7.2 Fold the cross-platform scroll convention (sign + notch + per-provider translation) into the
  *durable* docs — `dev-docs/pointer-input.md` § Scroll gains a "Direction across platforms" table and a
  chunking note. (It previously lived only in this change's `design.md`, which archives away.)
- [x] 7.3 Windows `HWHEEL` horizontal-sign (6.3): verify on a real desktop — **owner: user** (tested under
  Windows; folds into 5.5). A pure unit test for the sign is possible but low value next to the real run.
  → confirmed by the user on Windows: `Pointer Scroll RIGHT`/`LEFT` scroll the correct way.
- [x] 7.4 Descriptor-based acceptance case in `tests/acceptance/egui/scroll.robot` — scroll *over an
  element* (not coordinates), the real-platform counterpart to the mock move-to-target check. Green on the
  compositor (3/3).
