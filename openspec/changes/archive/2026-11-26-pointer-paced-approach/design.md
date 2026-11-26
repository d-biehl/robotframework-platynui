# Design

## Context

See proposal.md (Why) for the failure and the GTK 3 rule behind it. The requirements are in
`specs/pointer-motion/spec.md`.

How a move works today (verified in the code):

- `PointerEngine::move_to` (`crates/runtime/src/pointer.rs:342`) calls `perform_move` and then
  waits `after_move_delay`. Click, multi-click (`:372`), drag (`:469`) and a scroll with a target
  all reach the device through `perform_move`.
- `perform_move` (`crates/runtime/src/pointer.rs:539`) sends a `Direct` move as one
  `device.move_to`. Every other mode builds a path with `generate_path` (`:835`): about
  `steps_per_pixel` (≥ 1) points per pixel, each mode ending at the target. Overshoot ends with its
  settle steps back to the target (`:893`). Jitter's envelope tapers to zero at the end (`:905`).
  The points are sent on an eased schedule over `desired_move_duration` (`:583`), with nothing
  between the sleeps. With both duration knobs at 0, the path goes out as fast as the device
  takes it.
- A zero-distance move yields a one-point path, the target itself (`:837`).
- The defaults are in `PointerProfile::named_default` (`:57`): 1.5 steps per pixel, 800 µs per
  pixel, 600 ms cap, 40 ms `after_move_delay`. A 50 px move therefore sends about 75 points in
  about 40 ms.
- The unit tests drive the engine with a recording pointer device (`:1004`) and a sleep closure
  that records every duration (`:1230`, `:1464`, `:1520`). This makes the timing verifiable
  without a real display.
- The Python bindings read profile and override dicts with a key reader. Counts go through `count`
  (`packages/native/src/runtime.rs:1916`), which rejects anything that is not a non-negative
  integer with an error naming the key. Durations go through `duration_from_millis` (`:1975`),
  which by design clamps negative values to 0.
- The CLI parses delay flags as whole milliseconds with `parse_millis`
  (`crates/cli/src/commands/pointer.rs:540`).
- The high-level library mirrors the pointer delay fields as `pointer_*_ms` settings
  (`src/PlatynUI/core/settings.py:49`) and maps them in `src/PlatynUI/core/devices.py:59`.

The GTK 3 behavior comes from GTK's `gtk-3-24` sources (`gtkmenushell.c`, `gtkmenu.c`), not from
PlatynUI code:

- `MENU_SHELL_TIMEOUT` is 500 ms.
- `N_MOTION_EVENTS` is 3.
- `priv->n_motion_events` is reset in `gtk_menu_enter_notify` and counted in
  `gtk_menu_motion_notify`.

## Goals / Non-Goals

**Goals:**

- The last motion before a click reaches the toolkit as separate events over the target, even when
  the toolkit reads its input once per frame.
- One rule for every path mode and every pointer device. There is no toolkit detection.

**Non-Goals:**

- Changing `Direct` mode. It stays the documented single OS call.
- Guaranteeing delivery when a toolkit blocks its main loop longer than the approach interval. The
  approach makes merging unlikely, not impossible.
- Changing the path shapes, the easing curve, or `after_move_delay`.
- Keeping the approach when the profile asks for zero move time (see D3).

## Decisions

### D1 — The approach is the paced tail of the path, not extra points

`perform_move` splits the generated path into a body and an approach tail:

- **Building the tail.** Walk the path backwards from the target. Collect points whose rounded
  integer position differs from the point collected before. Stop after `approach_steps` points.
  The target is always the last point of the tail.
- **Sending the body.** The body is everything before the tail. It goes out on the existing eased
  schedule, over the part of the move's duration that the approach leaves (D3).
- **Sending the tail.** Each tail point goes out only after at least the effective interval (D3)
  has passed since the previous `device.move_to`. This is measured with `Instant`, so time already
  spent in the device call counts, and the first tail point is spaced from the last body point too.
- **Skipped points.** Path points between the collected tail points (same integer pixel as a
  collected one) are dropped. Sending them would only add events the toolkit merges anyway.

Alternatives considered:

- **Extra points after reaching the target** (a wiggle around it, or repeating the target).
  Rejected for two reasons:
  - A wiggle leaves the target, which matters for small targets.
  - A motion to the same integer pixel produces no event on X11, so a repeat would be invisible
    there.
- **Slower default `move_time_per_pixel`.** Rejected. It slows every move by much more, and the
  path is still sent in steps far below one frame apart.
- **Hovering in the test.** Rejected. It adds only about 130 ms, and the stress run with jumps
  still failed 3 of 3 times (proposal, Why).

### D2 — Distinct integer pixels, the target exact

The devices take `f64` points, but X11 (XTest) and Windows (`SendInput`) work in whole pixels.
Two approach steps that round to the same pixel would look like one step there. The tail
therefore uses the rounded position only to decide whether points are distinct. The points
themselves are sent unchanged, so the last one is exactly the target, including a fractional
target such as the centre of an odd-sized element.

A path shorter than `approach_steps` pixels yields fewer tail points. A zero-distance move yields
a tail of only the target, which is already where the pointer is. That move then waits for no
approach interval (spec: "nothing to approach").

### D3 — The approach sets a minimum move duration

The approach is part of the move's duration, not added on top. For a move whose tail has `n`
points:

- **Approach time.** `A = n × approach_interval`.
- **Path time.** `D` is what `desired_move_duration` returns today: per-pixel time, capped by a
  non-zero `max_move_duration`.
- **Total.** `T = max(D, A)`, then capped by `max_move_duration` when that is non-zero.
- **Split.** The effective interval is `min(approach_interval, T / n)`. The tail gets
  `n × effective interval`, and the body gets the rest of `T` (possibly 0, then it goes out as a
  burst before the paced tail).
- **Zero move time.** With `move_time_per_pixel` and `max_move_duration` both 0 (the profile asks
  for "as fast as possible"), there is no approach: `T = 0` and the tail goes out unpaced, as
  today.

Consequences:

- **Defaults.** A 50 px move has `D` = 40 ms and `A` = 60 ms. It takes 60 ms: the body as a burst,
  then 3 steps 20 ms apart. A 500 px move keeps its 400 ms, with the last 60 ms being the paced
  tail.
- **Short cap.** `max_move_duration` 30 ms with the default approach squeezes the 3 steps to 10 ms
  apart. The maintainer accepted that a short cap or zero move time reduces or removes the
  approach.
- **Unchanged profile.** The "Instant" profile in `dev-docs/pointer-input.md` §11 uses `Direct`, so
  it is unaffected anyway.

Alternatives considered:

- **On top of the move duration, never capped.** Rejected by the maintainer: the approach should
  respect the move duration and its cap.
- **Squeezed into the unchanged move duration.** Rejected. At the default 800 µs per pixel, every
  move under about 75 px would get intervals below one frame. A 50 px menu click would get about
  13 ms, which is exactly the case this change is for.

### D4 — Defaults: 3 steps, 20 ms apart

- **Why 3 steps.** GTK 3 needs 3 motion events over its items after the menu's last enter. The
  tail points lie within the last few pixels before the target, which is normally the middle of
  the element. That is after the pointer entered the menu, and the body's own motion inside the
  menu adds to the count.
- **Why 20 ms.** 20 ms is above one 60 Hz frame (16.7 ms), which leaves a margin for a toolkit
  that reads input once per frame.
- **Cost.** Only moves shorter than about 75 px get slower, by at most 60 ms (a move of zero
  distance gets no approach). Longer moves keep their duration. A default click already spends
  about 40 + 50 + 80 + 35 ms in its fixed delays.

The maintainer confirmed these defaults on 2026-11-26.

### D5 — Parameter names and units per surface

| Surface | Steps | Interval |
|---------|-------|----------|
| Rust (`PointerProfile`, `PointerOverrides`) | `approach_steps: u32` | `approach_interval: Duration` |
| Python (profile/override dicts and classes) | `approach_steps` (int) | `approach_interval_ms` (float ms) |
| CLI | `--approach-steps <n>` | `--approach-interval <ms>` |
| High-level library settings | `pointer_approach_steps` | `pointer_approach_interval_ms` |

On every surface, the conventions are the existing ones:

- Steps are read with the bindings' `count` reader, so a negative or fractional value is an error
  that names the key.
- The interval goes through `duration_from_millis`, so a negative value counts as 0 (and turns the
  approach off).
- On the CLI the interval is in whole milliseconds, like every other delay flag.

BareMetal needs no new keyword arguments. `pointer_profile` and `overrides` already take the
binding dicts and classes, so the new keys arrive through them. Only the `PointerOverridesLike` and
profile type hints gain the keys.

### D6 — Drag and scroll get the approach too

Drag and a scroll with a target reach the device through the same `perform_move`, so they get
the approach without extra code. That is wanted: a drop target or a scroll container is the same
kind of "toolkit reacts to motion over me" target. The spec's first requirement names them.

## Risks / Trade-offs

- **[Short moves get up to 60 ms slower]** → Mitigation: long moves are unchanged (D3), and the
  approach can be turned off per profile or per action. The acceptance lanes grow by a few seconds
  overall. Documented in `dev-docs/pointer-input.md` §6/§7/§10.
- **[A short `max_move_duration` squeezes the intervals below a frame]** → Accepted by the
  maintainer. The cap is an explicit request for speed, and the documentation says what it costs.
- **[A toolkit that blocks longer than 20 ms can still merge approach steps]** → Mitigation: the
  body's own motion inside the target adds to GTK's count. The interval and the step count are
  tunable. The spec promises spacing, not delivery.
- **[Existing runtime unit tests assert exact move counts or sleep sequences]** (for example around
  `crates/runtime/src/pointer.rs:1230`, `:1464`, `:1520`) → Mitigation: each is updated to the new
  expected sequence. A test that checks something unrelated to the approach sets
  `approach_steps` 0, so it keeps testing only its own subject.
- **[Hover-sensitive widgets see up to 3 more motion events]** → Accepted. They were already
  getting dozens per move.
- **[The improvement is confirmed only for GTK 3]** → Accepted. The mechanism (merging per frame)
  is generic, but the spec's real-toolkit scenario is the GTK 3 menu. The other acceptance lanes,
  including Windows on the VM, only have to stay green.

## Migration Plan

- **Kind of change:** behavioral (pointer timing), additive in the API (two new parameters), not
  BREAKING.
- **Native rebuild:** needed, `just build-native`. The bindings and the CLI both change.
- **Rollout:** one change across runtime, bindings, CLI, library settings and docs.
- **Rollback:**
  - For users, setting `approach_steps` to 0 in the profile restores the old timing exactly.
  - For the project, revert the change.

## Resolved Questions

The maintainer decided these on 2026-11-26, before implementation:

- **Defaults:** 3 steps, 20 ms apart (D4).
- **`Direct`:** stays a single jump, with no approach.
- **Duration:** the approach is part of the move's duration and sets a minimum duration (D3). Only
  a non-zero `max_move_duration` shortens it, and zero move time removes it. This replaced the
  first draft, where the approach came on top of the duration and was never capped.
- **Invalid values:** handled like the other pointer parameters. A negative interval counts as 0;
  a negative or fractional step count is an error naming `approach_steps`.
