# Tasks

## 1. Shared frame geometry

- [ ] 1.1 Move `expand_rect` and `frame_segments`, with their constants and unit tests, from `crates/platform-linux-x11/src/highlight.rs` into `crates/core/src/platform/highlight.rs`. Make the X11 provider call them from there. Verify that the moved tests pass under `just test-crate platynui-core`, that `just test-crate platynui-platform-linux-x11` stays green, and that the X11 lanes (`just headless=true test-acceptance-x11`) run their highlight calls unchanged.

## 2. Tests first

- [ ] 2.1 Add unit tests in `crates/platform-linux-wayland/src/highlight.rs` for:
  - the desktop-to-output mapping: one output; two outputs side by side, with a rectangle cut at the shared edge and no gap; an output at a negative position;
  - the availability decision: globals with and without `zwlr_layer_shell_v1`; the error names the layer shell and the compositor.

  Verify that they fail before section 3.
- [ ] 2.2 Add an integration test in `crates/platform-linux-wayland/tests/` that starts a headless PlatynUI compositor with two 1920×1080 outputs, following the harness pattern of `tests/pidns_tests.rs`. Highlight through `platynui-cli highlight` and capture through `platynui-cli screenshot`. Assert the spec's scenarios:
  - frame pixels for (100, 100, 200, 50) and the rectangle's own pixels unchanged;
  - the frame across two outputs for (1800, 100, 240, 50);
  - no frame after a 0.5 s duration has passed;
  - only the second frame after a replacement;
  - no frame right after clearing.

  Verify that it fails on the current code, where the frame is drawn by the compositor: the pixels would appear, so the test also asserts that the compositor received no `show_highlight` command (its debug log). It must fail on that assertion.
- [ ] 2.3 Add compositor-lane acceptance tests to `tests/acceptance/egui/interaction.robot`, following the `robot-test-style` skill:
  - "A Click Goes Through A Highlight": highlight `btn-click-me` for 10 s, then click it and wait for the click counter to increase;
  - "Typing Reaches The Focused Field While Its Window Is Highlighted".

  Verify that they pass on the current code. The compositor-drawn frame takes no input either, so they guard the new surface's input and focus behavior and are not expected to fail first.

## 3. Layer-shell overlay

- [ ] 3.1 Implement the overlay thread (design D1):
  - its own connection;
  - binding `wl_compositor`, `wl_shm`, `zwlr_layer_shell_v1`, and the outputs with `zxdg_output_v1`;
  - command and acknowledgement channels, so errors reach the caller;
  - start on the first highlight and stop on drop.

  Verify with the 2.1 availability tests.
- [ ] 3.2 Implement the surfaces and drawing (design D2): one full-output overlay surface per reached output, with anchors on all edges, exclusive zone `-1`, no keyboard interactivity and an empty input region. Draw an `ARGB8888` `wl_shm` buffer in the logical output size from the shared frame segments, translated per output. Verify that the 2.1 mapping tests and the frame scenarios of 2.2 pass.
- [ ] 3.3 Implement duration, replacement and clearing (design D4). Verify that the remaining 2.2 scenarios pass.
- [ ] 3.4 Switch `WaylandHighlightProvider` to the overlay and drop the control-socket calls and the identification-based gate (design D5). Verify that 2.2's no-`show_highlight` assertion passes, that the 2.3 tests and the egui smoke suite's `BM.Highlight` calls pass in `just headless=true test-acceptance-compositor`, and that clearing still succeeds without a layer shell (2.1).

## 4. Documentation

- [ ] 4.1 Update the docs:
  - `dev-docs/platform-linux-wayland.md` §7.3 (implemented, how, the Mutter gap) and §15 (the roadmap row for `HighlightProvider`);
  - `dev-docs/architecture.md` §10 (the platform snapshot).

  Verify by comparing each statement with the spec.

## 5. Verification

- [ ] 5.1 Run `just check`, `just test` and `just test-python`; all must be green.
- [ ] 5.2 Run `just headless=true test-acceptance-compositor`, `test-acceptance-x11` and `test-acceptance-x11-bare`; all must be green.
- [ ] 5.3 Manual check on a foreign compositor: run `platynui-cli highlight` with a rectangle and `--duration-ms 3000` in a nested niri or sway session (`scripts/startsway.sh`). The frame must appear at the rectangle and clicks must go through it. Record the result here.

## Workflow follow-up

- Commit and push only on the maintainer's explicit go-ahead.
- Archive after CI is green.
