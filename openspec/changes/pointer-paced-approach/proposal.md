# Proposal

## Why

PlatynUI moves the pointer much faster than a person does: a 50 px move is about 78 motion steps
delivered in about 40 ms. Toolkits that read input once per frame merge such a burst into one or two
motion events. GTK 3 menus need at least three motion events over their items before a release that
comes less than 500 ms after the press that opened the menu counts as a click
(`MENU_SHELL_TIMEOUT` and `N_MOTION_EVENTS` in `gtkmenushell.c`/`gtkmenu.c`). A click that follows
straight after opening a GTK 3 menu is therefore swallowed. The menu stays open, the item is
highlighted, and nothing is activated.

This made the compositor acceptance lane fail twice in a row in CI on the GTK 3 test
`Activating A Menu Bar Item Updates The Last Action` (run 45704167967, after the smithay git pin).
The same failure appeared once in the X11 lane on 2026-11-17, so it is not specific to one display
server. It reproduces locally every time with a pointer jump instead of a path, and passes once the
click comes 600 ms later. The compositor logs of the passing and failing runs match line by line,
which places the cause in how PlatynUI delivers motion, not in the compositor. A test-side
workaround (hovering the item before clicking) adds only about 130 ms and still fails when the
motion arrives merged.

## What Changes

- Every pointer move along a path (the `Linear`, `Bezier`, `Overshoot` and `Jitter` motion modes)
  ends with a **paced approach**: the last few motion steps before the target are delivered at
  least one approach interval apart, each at its own integer pixel, and the last one lands exactly
  on the target. A toolkit that merges per frame then still sees separate motion over the target
  before the click.
- Two new pointer-profile parameters control the approach: `approach_steps` (number of paced steps
  at the end of a move) and `approach_interval` (minimum time between them). Setting either to 0
  turns the approach off. Both can be set on the profile, overridden per action, and set from the
  CLI and from Robot Framework, like every other pointer timing parameter.
- The `Direct` motion mode keeps its documented meaning: one OS call, with no intermediate steps
  and no approach.
- A move of zero distance (the pointer is already on the target) gets no approach.
- **Behavior change, not BREAKING:** pointer moves get slightly slower by default (by about
  `approach_steps × approach_interval`, see design). Scripts that need the old timing set
  `approach_steps` to 0.

## Capabilities

### New Capabilities

- `pointer-motion`: How the runtime turns a pointer move into motion events: the path, its timing,
  and the paced approach at its end, with the knobs that control it on every surface (runtime
  profile and overrides, Python bindings, CLI, Robot Framework).

### Modified Capabilities

(none — no existing spec covers pointer motion; `baremetal-pointer-scroll` covers only the wheel)

## Impact

- **Rust runtime:** `crates/runtime/src/pointer.rs`: `PointerProfile`, `PointerOverrides`,
  `EffectiveProfile` and `perform_move` (also used by `drag`), plus their unit tests with the
  recording device and the recorded-sleep fixture.
- **CLI:** `crates/cli/src/commands/pointer.rs`: new override flags.
- **Python bindings:** `packages/native/src/runtime.rs` (the `PointerOverrides` class and the
  profile dict conversion), `packages/native/python/platynui_native/_native.pyi` and `__init__.py`
  (TypedDicts), `packages/native/tests/test_overrides_classes.py`. This **needs a native rebuild**
  (`just build-native`).
- **Robot Framework library:** `src/PlatynUI/core/settings.py` and `src/PlatynUI/core/devices.py`
  (library-wide pointer settings), plus the `PointerOverridesLike` keys accepted by the BareMetal
  pointer keywords.
- **Docs:** `dev-docs/pointer-input.md` (timing table, click timeline, speed profiles). The
  user-facing docs mention the parameters where pointer timing is described.
- **Tests:** runtime unit tests (mock-verifiable timing), Python binding tests, and the existing
  GTK 3 acceptance test as the real-toolkit check in the compositor and X11 lanes. No acceptance
  suite changes are planned.
- **Platforms:** the approach is platform-independent runtime logic and applies to every pointer
  device (Windows, X11, Wayland compositor, EIS/portal, mock). How much it matters depends on the
  toolkit: it targets toolkits that merge motion per frame, GTK 3 menus being the confirmed case.
