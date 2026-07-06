## Why

The BareMetal library exposes pointer move, click, multi-click, press, release and position — but **not scrolling**, even though the native runtime (`Runtime.pointer_scroll`) and the CLI (`platynui-cli pointer scroll`) already support it. `dev-docs/pointer-input.md` even states the pointer API is *"consistent across CLI, Python, and Robot Framework"* and lists scrolling — so the Robot Framework surface is the one place that contradicts the documented intent. Tests that need to reach content below the fold (long lists, panels, dialogs) currently have no way to scroll.

## What Changes

- Add a **`Pointer Scroll`** keyword to the BareMetal library that turns the mouse wheel by a number of notches in a direction, over an element / at coordinates / at the current pointer position. The keyword itself is a thin wrapper over the existing `Runtime.pointer_scroll`. But it is the *first* code path to actually emit scroll through the real platforms, and doing so surfaced latent, never-exercised bugs in the platform scroll injection — so making scroll observably work end-to-end also required platform-layer fixes (chunking, Wayland compositor, Windows provider; see Impact).
  - Targeting mirrors the rest of the pointer family: `descriptor` first (pass `${None}` to scroll at the current position), plus keyword-only `x`/`y`, `overrides`, `activate`, `query_overrides`. When a target is given, the pointer is moved over it first (wheel events go to the widget under the cursor).
  - Scroll amount is expressed as `direction` (`UP`/`DOWN`/`LEFT`/`RIGHT`, default `DOWN`) and `ticks` (mouse-wheel notches, default 1; one notch = 120 units). The keyword owns the sign/axis convention so the test author never deals with signed deltas. No raw delta argument for now (YAGNI; addable later without breaking the API).
- Add a **dedicated scroll box** to the egui test app (`apps/test-app-egui`) so the real scroll effect is observable in the acceptance lane: a fixed-size `ScrollArea` with overflowing content plus an `@Id`-tagged label that reports the current scroll offset.
- Update `dev-docs/pointer-input.md` with the Robot Framework scroll usage (making the "consistent across … Robot Framework" claim true).

Out of scope: a `Pointer Drag` keyword — drag-and-drop is already expressible via `Pointer Press` → `Pointer Move To` → `Pointer Release`, so a convenience wrapper is deliberately deferred.

## Capabilities

### New Capabilities
- `baremetal-pointer-scroll`: the BareMetal `Pointer Scroll` keyword — its targeting model, the direction/ticks amount model, and the wheel-notch unit convention.

### Modified Capabilities
<!-- None: the existing pointer keywords are not yet spec'd and are not respecified here. -->

## Impact

- **Affected specs:** `baremetal-pointer-scroll` (new).
- **Python:** `src/PlatynUI/BareMetal/__init__.py` — one new keyword (+ a `ScrollDirection` value type) and docstring. Reuses the existing point-resolution / window-activation / query-settings machinery unchanged.
- **Rust (test fixture):** `apps/test-app-egui` — a scroll box with an offset indicator for acceptance observability.
- **Rust (platform scroll fixes, exposed by the first real scroll):**
  - `crates/runtime/src/pointer.rs` — scroll chunking fell back to 1-unit micro-steps for an axis whose `scroll_step` component is 0 (horizontal defaults to 0), which broke horizontal scroll on every platform; now falls back to one notch (120) per step.
  - `apps/wayland-compositor/src/control.rs` — `inject_pointer_scroll` now injects `AxisSource::Wheel` + discrete `v120` (was `Finger` + continuous value) and translates PlatynUI's sign convention to Wayland's.
  - `crates/platform-windows/src/pointer.rs` — negate the horizontal delta for `MOUSEEVENTF_HWHEEL` (Win32's horizontal wheel is right-positive, opposite to PlatynUI/X11).
- **Tests:** a pytest for the direction/ticks → `ScrollDeltaLike` mapping; an RF mock suite for wiring/resolution and move-to-target; an egui acceptance suite that scrolls the new box and asserts the offset via `Wait Until Query` (egui scrolls smoothly, so the assertion waits for the offset to settle).
- **Docs:** `dev-docs/pointer-input.md` gains the Robot Framework scroll examples.
