# Tasks

## 1. Runtime: paced approach in the pointer engine

- [x] 1.1 Write the unit tests for the spec's runtime scenarios in `crates/runtime/src/pointer.rs`,
      using the recording pointer device and the recorded-sleep closure:
      - a 50 px `Linear` move ends with 3 steps ≥ 20 ms apart, each at its own integer pixel, the
        last exactly at the target;
      - a click finishes the approach before the press;
      - `Bezier`, `Overshoot` and `Jitter` end with the same approach;
      - `Direct` sends one move and waits no approach interval;
      - a zero-distance move or click waits no approach interval;
      - a 1 px move sends exactly one paced step;
      - `approach_steps` 0 and `approach_interval` 0 each turn the approach off;
      - a per-action override of 5 steps at 30 ms applies once, and the next move uses the profile
        again;
      - duration (design D3): a 50 px move takes at least 60 ms, a 500 px move keeps its 400 ms,
        `max_move_duration` 30 ms spaces the steps evenly and at least 10 ms apart, and zero move
        time (both duration knobs 0) waits no approach interval.
      Verify: they fail before 1.2 (they do not compile yet, or they assert the missing spacing).
- [x] 1.2 Add `approach_steps` (default 3) and `approach_interval` (default 20 ms) to
      `PointerProfile`, add the `PointerOverrides` builder methods and the `EffectiveProfile`
      accessors. Implement the body/tail split and the pacing in `perform_move` as in design
      D1–D3. Verify: the tests from 1.1 pass with `just test-crate platynui-runtime`.
- [x] 1.3 Update the existing pointer unit tests whose exact move counts or sleep sequences change
      (around `crates/runtime/src/pointer.rs:1230`, `:1464`, `:1520`). A test whose subject is not
      the approach sets `approach_steps` 0. Verify: `just test-crate platynui-runtime` is fully
      green, and `just check` shows no clippy findings in `platynui-runtime`.
- [x] 1.4 Document the approach in `dev-docs/pointer-input.md`:
      - §4: `Direct` has no approach;
      - §6: the approach is part of the move duration: it sets a minimum duration, a non-zero
        `max_move_duration` caps the whole move, and zero move time means no approach;
      - §7: both parameters in the profile timing table, with the per-layer names and units;
      - §10: the click timeline shows the approach before `after_move_delay`;
      - §11: the default profile lists the approach.
      Verify: the documented defaults match `PointerProfile::named_default`.

## 2. CLI

- [x] 2.1 Add `--approach-steps <n>` and `--approach-interval <ms>` to the pointer command overrides
      in `crates/cli/src/commands/pointer.rs`, with a test that the flags reach the overrides.
      Document both flags in the CLI flag list of `dev-docs/pointer-input.md` §9 (and in
      `dev-docs/cli.md` if it lists pointer flags). Verify: `just test-crate platynui-cli` passes,
      and `platynui-cli pointer move --help` lists both flags.

## 3. Python bindings (native rebuild)

- [x] 3.1 Extend `packages/native/tests/test_overrides_classes.py` (and the profile tests next to
      it) first:
      - `approach_steps` / `approach_interval_ms` round-trip through the `PointerOverrides` and
        `PointerProfile` classes and through the dicts;
      - a negative or fractional `approach_steps` raises an error that names `approach_steps`;
      - a negative `approach_interval_ms` reads back as 0.
      Verify: they fail before 3.2.
- [x] 3.2 Add the keys to `packages/native/src/runtime.rs`: the `PointerOverrides` kwargs and
      getters, the `PointerProfile` kwargs and getters, and the dict readers (steps with `count`,
      interval with `duration_from_millis`). Add them to the TypedDicts and classes in
      `packages/native/python/platynui_native/_native.pyi` and `__init__.py`. Rebuild with
      `just build-native`. Verify: the tests from 3.1 pass under `just test-python`.

## 4. Robot Framework library

- [x] 4.1 Add a mock-lane suite `tests/BareMetal/pointer_approach.robot` (following the
      `robot-test-style` skill, modelled on `pointer_scroll.robot`) with these cases:
      - `Pointer Click` with `overrides=${{ {"approach_steps": 0} }}` lands on its target, checked
        with `Get Pointer Position`;
      - an override with a negative `approach_steps` fails with an error naming `approach_steps`;
      - a library imported with `pointer_profile` that sets the approach parameters clicks
        normally.
      Verify: `just test-baremetal --suite PointerApproach` passes (after 3.2).
- [x] 4.2 Add `pointer_approach_steps` and `pointer_approach_interval_ms` to
      `src/PlatynUI/core/settings.py`, map them in `src/PlatynUI/core/devices.py`, and extend the
      mapping tests in `tests/PlatynUI/test_devices.py` next to the `pointer_after_move_delay_ms`
      case. Verify: `just test-python` passes, and `just check` (ruff, mypy) is clean.

## 5. Verification across the real lanes

- [x] 5.1 Run the full gate: `just check`, `just test` and `just test-python` are green.
- [x] 5.2 Check the spec's burst scenario locally. Temporarily give the `menu-file-new` click in
      `tests/acceptance/gtk3/catalog.robot` the override `move_time_per_pixel_us` 1 (the path
      before the approach goes out as one burst), then run
      `just headless=true test-acceptance-compositor --suite Tests.Acceptance.Gtk3.Catalog --test ActivatingAMenuBarItemUpdatesTheLastAction`
      and the same with `test-acceptance-x11`. Verify: the test passes in both lanes. Revert the
      temporary edit and confirm `git status` shows no change to that file. A local machine reads
      the burst fast enough that GTK 3 does not merge it, so the test also passes there with
      `approach_steps` 0 (checked on one CPU core too). Locally this only shows that the approach
      keeps the click working. The discriminating check is the CI compositor lane, where the test
      failed twice without the approach (Workflow follow-up).
- [x] 5.3 Run the full compositor, X11 and X11-bare acceptance lanes headless locally. Verify: all
      green, apart from the known skip.
- [ ] 5.4 Run the acceptance lane on the Windows VM (real Windows, not Wine). Verify: green, so the
      approach does not disturb UIA, JAB or the Win32 tests.

## Workflow follow-up

- Commit and push only on the maintainer's explicit go-ahead.
- After pushing: CI's "Acceptance (Linux, compositor)" job passes
  `Activating A Menu Bar Item Updates The Last Action`. That also unblocks task 7.3 of
  `migrate-compositor-smithay-git`, whose archive waits for green CI.
- Archive `pointer-paced-approach` once CI is green, then finish and archive
  `migrate-compositor-smithay-git`.
