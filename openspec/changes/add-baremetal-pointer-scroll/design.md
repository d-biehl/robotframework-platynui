## Context

BareMetal's pointer keywords (`Pointer Click`, `Pointer Multi Click`, `Pointer Press`, `Pointer Release`, `Pointer Move To`, `Get Pointer Position`) cover everything the native pointer device offers **except scrolling**. The capability is already there underneath:

- `Runtime.pointer_scroll(delta: ScrollDeltaLike, overrides)` exists in the native binding (`ScrollDeltaLike` is the `(h, v)` tuple); `scroll_step` (default `(0, -120)`) and `scroll_delay_ms` are part of the pointer settings/overrides.
- The CLI exposes `platynui-cli pointer scroll 0,-360` (and `--expr` to scroll over an element).
- `dev-docs/pointer-input.md` already documents scrolling and claims the API is "consistent across CLI, Python, and Robot Framework" — so the RF gap is a doc-vs-code divergence to close, not new ground in the core.

One mouse-wheel notch is **120 units** (`WHEEL_DELTA`, a cross-platform convention — Win32's constant, Wayland's `axis_value120`); down is negative (`(0, -360)` = three notches down). `pointer_scroll` scrolls at the **current** pointer position, so scrolling a specific widget means moving over it first (what the CLI's `--expr` does).

Testing is the interesting part: the mock UI tree is static (scrolling changes nothing there) and the mock pointer device records scroll into a log (`PointerLogEntry::Scroll`) that is **not exposed to Python**. egui, however, has a real `ScrollArea`, and its `ScrollArea::show(...) -> ScrollAreaOutput` exposes `state.offset` (verified on docs.rs) — so the real effect is observable through an `@Id`-tagged label. egui scrolls **smoothly/kinetically** (`smooth_scroll_delta`, `State::velocity()`), so the offset animates over several frames and must be asserted with a waiting read.

## Goals / Non-Goals

**Goals:**
- Add a single, readable `Pointer Scroll` keyword consistent with the rest of the pointer family.
- Express the amount in mouse-wheel notches and own the sign/axis convention so test authors never write signed deltas.
- Make the real scroll effect deterministically testable in the acceptance lane.
- Make `dev-docs/pointer-input.md`'s "consistent across … Robot Framework" claim true.

**Non-Goals:**
- No `Pointer Drag` keyword (drag-and-drop is already expressible via press/move/release).
- No raw `delta_x`/`delta_y` argument for now (LEFT/RIGHT cover horizontal; pixel/diagonal scrolling is not needed in UI tests). Addable later without breaking the API.
- No change to the native pointer runtime (scroll already exists there).

## Decisions

**D1 — One keyword, descriptor-first, scroll specifics keyword-only.** `pointer_scroll(descriptor=None, *, direction=DOWN, ticks=1, x=None, y=None, overrides=None, activate=None, query_overrides=None)`. `descriptor` stays the first positional like the whole pointer family (`${None}` to scroll at the current position). `direction`/`ticks` are keyword-only because a positional string would collide with the `UiNodeDescriptor` string-converter in the first slot (the same RF binding pitfall seen with `Wait Until Query`).

**D2 — Amount as `direction` + `ticks`, not raw delta.** `direction` ∈ {`UP`,`DOWN`,`LEFT`,`RIGHT`} (default `DOWN`); `ticks` is the number of wheel notches (default 1, one notch = 120 units). The keyword maps these to the native `(h, v)` delta and owns the sign/axis convention (`DOWN` = visually down regardless of `scroll_step` defaults). Rationale: raw signed deltas (`delta_y=-360`) are a double footgun (sign + the 120 magnitude); `LEFT`/`RIGHT` already cover horizontal.

**D3 — The amount argument is named `ticks`.** `clicks` is rejected (already means *button* clicks in `Pointer Multi Click` — same family, different meaning); `steps` is rejected (collides with `scroll_step`, which controls the stepping animation, not the distance). `ticks` ("wheel ticks") is unambiguous and collision-free.

**D4 — Reuse the existing pointer plumbing.** Resolve the point with `_resolve_screen_point` and raise the window with `_maybe_bring_to_front` exactly like the other pointer keywords; when a point is resolved, `pointer_move_to` over it, then `pointer_scroll(delta, overrides)`. `overrides` already carries `scroll_step`/`scroll_delay_ms` (and the move motion); `activate` and `query_overrides` behave as elsewhere.

**D5 — Layered test strategy.** The static mock tree and the unexposed scroll log mean no single lane covers everything, so split by concern:
- *pytest* with a fake runtime asserts the `direction`+`ticks` → `ScrollDeltaLike` mapping (sign, axis, ×120) — the new logic, tested in isolation.
- *RF mock* asserts wiring/resolution (missing target → not-found error honoring `query_overrides`) and move-to-target (`Get Pointer Position` equals the element's point).
- *egui acceptance* asserts the real, reversible effect against a dedicated scroll box.

**D6 — A dedicated scroll box in the egui test app.** Add a fixed-size `ScrollArea` (so content always overflows — no dependence on window size) of non-scroll-consuming label rows (so the wheel reaches the area, not a value widget), plus an `@Id`-tagged label outside the box rendering `output.state.offset`. The acceptance test scrolls over the box and asserts **direction + reversibility** of the offset (not exact pixels — egui maps wheel units to points device-dependently), using `Wait Until Query` to wait out the smooth-scroll animation. A both-axes box lets one fixture cover all four directions.

**D7 — `Pointer Drag` is out of scope.** It is only a convenience over `Pointer Press` → `Pointer Move To` → `Pointer Release`; deferred deliberately.

## Risks / Trade-offs

- Horizontal sign convention (`RIGHT`/`LEFT`) is not documented in `dev-docs/pointer-input.md` → the keyword owns it. Resolved during implementation from the X11 wheel-button mapping (`crates/platform-linux-x11/src/pointer.rs`: +v→button 4 = up, −v→button 5 = down; +h→button 6 = left, −h→button 7 = right), which gives one consistent rule — a *negative* component increases the offset on each axis, so `RIGHT` is `−h` (and `DOWN` is `−v`, matching the established `scroll_step` default). This corrects the draft's `RIGHT → +h`. Isolated in one `_SCROLL_AXIS_SIGN` table; the egui acceptance suite asserts the visible direction on both backends, so a platform that disagrees fails loudly and is a one-row flip.
- egui smooth/kinetic scrolling → the offset is not final immediately; acceptance must assert with a waiting read (`Wait Until Query`), never a single immediate `Get Attribute`.
- The mock scroll log is not exposed to Python → the exact emitted delta can't be asserted in the RF mock lane, hence the pytest. (A future, broader option: expose `take_pointer_log()` on the runtime to make *all* pointer keywords effect-testable in the mock lane — out of scope here.)
- `ScrollDirection` representation: a `Literal["UP","DOWN","LEFT","RIGHT"]` (RF converts the string) vs a small enum with a converter — pick the lightest that gives good libdoc and a clear error on a bad value.

## Open Questions

- ~~Exact `ScrollDirection` type (Literal vs enum + converter)~~ — **resolved:** a `Literal['UP','DOWN','LEFT','RIGHT']` alias, matching the existing `scope: Literal[...]` house style; RF converts and validates it with a clear invalid-value error.
- Confirm the egui offset API path (`ScrollAreaOutput.state.offset`) and the both-axes scroll behavior when building the fixture (docs.rs indicates `state.offset` is correct).
- ~~Confirm the horizontal sign against the real platform~~ — **resolved** from the X11 wheel-button convention (see Risks); the egui acceptance suite is the final check on both backends.

## Platform scroll injection (discovered during implementation)

The keyword is a thin wrapper, but it is the first thing to *emit* scroll through the real platforms, and
the acceptance suite exposed three latent platform-layer bugs. Recording the model here because it is
non-obvious and spans the whole stack.

**One keyword convention, per-provider translation.** The keyword uses a single, platform-independent
convention — `DOWN`/`RIGHT` are *negative* deltas (`scroll_step` defaults to `(0, -120)`; Win32's
`WHEEL_DELTA` and X11's wheel buttons informed it). Each platform provider then translates that convention
to its native scroll API:

| Platform | Vertical (`DOWN = −v`) | Horizontal (`RIGHT = −h`) |
| --- | --- | --- |
| X11 (`platform-linux-x11`) | −v → button 5 (down) ✓ native | −h → button 7 (right) ✓ native |
| Wayland compositor (`apps/wayland-compositor` control socket) | negate → wl `+v` (down) | negate → wl `+h` (right) |
| Windows (`platform-windows`) | `WHEEL` `−v` (down) ✓ native | **negate** → `HWHEEL` `+h` (right) |

So X11 needs no sign translation (its button map already matches); Wayland negates both axes; Windows
negates only horizontal (its vertical `WHEEL` already matches, but `HWHEEL` is right-positive, opposite to
X11). The keyword's own sign table is therefore the *contract*, and each provider is responsible for making
"RIGHT scrolls right / DOWN scrolls down" true on its stack.

**The chunking bug (the real horizontal blocker).** `Runtime::scroll` chunks a delta into animation steps by
dividing by `scroll_step`. For an axis whose `scroll_step` component is 0 — horizontal, by default — the
fallback produced **1-unit** steps, i.e. hundreds of sub-notch events per scroll. On Wayland those are
`v120(1)` ≈ nothing; on X11 each rounds to `round(1/120) = 0` wheel clicks → horizontal never moved at all.
Fixed by falling back to one notch (120) per step when the axis has no configured step size. This was the
actual reason horizontal appeared "dead" while vertical worked, independent of any sign question.

**The Wayland source-type bug.** The compositor injected scroll as `AxisSource::Finger` (touchpad) with only
a continuous value. Injected mouse-wheel scroll should be `AxisSource::Wheel` with a discrete `v120` amount
(matching real wheel input and the existing EIS discrete handler); switched accordingly.

**Verification.** egui acceptance passes both directions on the Wayland compositor and X11 (headless and
visible). The Windows `HWHEEL` fix is reasoned + cross-compiled (`x86_64-pc-windows-gnu`) but not
runtime-verified (no Windows host here) — `just test-acceptance-windows` on a real desktop is the check.
