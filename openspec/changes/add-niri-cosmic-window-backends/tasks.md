# Tasks

## 1. Record the compositors' real answers

- [ ] 1.1 Start niri 26.04 nested and record its `Windows`, `Workspaces` and `Outputs` replies as JSON fixtures under `crates/platform-linux-wayland/tests/fixtures/niri/`. Record each of these situations:
  - a tiled window;
  - a floating window;
  - a minimized window;
  - a maximized window;
  - a window outside the workspace view;
  - two outputs.

  Also record what a `MoveFloatingWindow` with fixed coordinates does to the window's reported position. Write the resulting state, restore and move mapping into design D2 and close its open question. Verify that the fixtures exist and parse with `jq`.
- [ ] 1.2 Capture a `WAYLAND_DEBUG=1` trace of cosmic-comp 1.11 with one window on a second output that is minimized, restored and maximized. Turn it into decoded event sequences for the cosmic state model: toplevel added, title, app ID, state, and geometry with output. Store them as test data. Verify that the sequences cover the bounds, matching and closed-window scenarios of the spec.
- [ ] 1.3 Check whether the RemoteDesktop portal in a nested cosmic session can grant pointer input without a consent dialog. Record the answer in design D7, and with it which of D7's two cosmic lane scopes applies.

## 2. Identification and input

- [ ] 2.1 Add unit tests to `capabilities.rs` and `input/mod.rs`:
  - `niri` and `cosmic-comp` peer binaries classify as `Niri` and `Cosmic`;
  - `XDG_CURRENT_DESKTOP` values `niri` and `COSMIC` classify the same when the peer is not visible;
  - a PlatynUI handshake still wins over a `niri` desktop;
  - the input candidates are `[VirtualInput, Portal]` for niri and `[Portal, VirtualInput]` for cosmic.

  Verify that they fail before 2.2.
- [ ] 2.2 Add `CompositorType::Niri` and `CompositorType::Cosmic`, their classification, the input candidates, and their names in `unsupported_compositor` and the identification log record. Verify with `just test-crate platynui-platform-linux-wayland`.

## 3. Shared window matching

- [ ] 3.1 Move `match_best_window` out of `window_manager/platynui_ipc.rs` into a shared module. It works over a candidate view of optional process ID, title and size, and its error names what it compared. Add tests for candidates without a process ID:
  - equal title and size are ambiguous, with an error naming both;
  - a unique size decides;
  - an exact title wins over a contained one.

  Verify that the existing PlatynUI backend tests and the compositor acceptance lane's window suites stay green.

## 4. niri backend

- [ ] 4.1 Write unit tests against the 1.1 fixtures:
  - bounds including the output position (the spec's (2024, 54) case);
  - a window outside the view is unavailable;
  - a reply without `layout` names the niri version needed;
  - moving a tiled window is unavailable, and no request is sent (checked with a recording transport);
  - an operation on a window niri no longer lists fails with "window no longer exists";
  - two equal windows are told apart by process ID;
  - the popup query is unavailable.

  Verify that they fail before 4.2.
- [ ] 4.2 Implement `window_manager/niri_ipc.rs` as in design D2:
  - a socket transport behind a small trait, so the tests can record requests;
  - tolerant `serde` types;
  - resolve, geometry, is_active and state;
  - activate, close, minimize, maximize, restore, resize, and move for floating windows only.

  Dispatch it from `backend()`. Verify that the 4.1 tests pass.

## 5. cosmic-comp backend

- [ ] 5.1 Vendor `cosmic-toplevel-info-unstable-v1.xml` and `cosmic-toplevel-management-unstable-v1.xml` from cosmic-protocols 0.2.0 under `crates/platform-linux-wayland/protocols/`, unchanged and with their copyright and permission notices. Generate client bindings with `wayland-scanner`'s macros. Verify that the crate compiles, that `cargo tree --workspace -i wayland-scanner` shows no new crate, and that no GPL crate enters the dependency tree.
- [ ] 5.2 Write unit tests on the cosmic state model, fed with the 1.2 sequences:
  - bounds including the output position (the spec's (1950, 40, 800, 600) case);
  - equal windows are ambiguous;
  - a unique size decides;
  - move and resize are unavailable;
  - the popup query is unavailable;
  - a closed handle fails with "window no longer exists";
  - a compositor without `zcosmic_toplevel_info_v1` version 2 fails with an error naming it.

  Verify that they fail before 5.3.
- [ ] 5.3 Implement `window_manager/cosmic_toplevel.rs` as in design D5:
  - the background connection, which binds the foreign-toplevel list, cosmic toplevel info and manager, and the outputs with `zxdg_output_v1`;
  - the state model;
  - a roundtrip before each read;
  - activate, close, minimize, maximize and restore through the manager.

  Dispatch it from `backend()`. Verify that the 5.2 tests pass.

## 6. Local lanes

- [ ] 6.1 Add `scripts/startniri.sh` and `scripts/startcosmic.sh` after `scripts/startsway.sh`. Each gets a private `XDG_RUNTIME_DIR`, D-Bus and AT-SPI bus, and the nested compositor; niri gets a generated config with `prefer-no-csd` and animations off. Each fails with a message naming the binary when the compositor is not installed. Verify that each starts a session in which `platynui-cli` lists the windows of a started egui test app.
- [ ] 6.2 Tag the suites that need the PlatynUI compositor with `compositor:platynui`. Find them by their use of the control socket, the popup list or compositor-only features. Candidates found during planning include several `tests/acceptance/egui` suites (for example `auto_activate`, `hit_test` and `config_display`). Also tag the suites that click GTK menu items. Add the `real-niri` and `real-cosmic` profiles to `robot.toml` and the `test-acceptance-niri` and `test-acceptance-cosmic` recipes to the `justfile`. Verify with `robotcode discover` that each profile selects the expected suites, and that the `real-wayland` selection is unchanged.
- [ ] 6.3 Make sure a compositor-independent suite covers the spec's lane scenarios:
  - telling two equal egui windows apart under niri;
  - clicking an egui button;
  - minimize, activate, maximize, restore and resize under niri;
  - minimize, activate, maximize and restore under cosmic;
  - operating on a closed window.

  Add tests where the existing egui suites do not cover them, following the `robot-test-style` skill. Verify by running them in the 6.4 lanes.
- [ ] 6.4 Run `just test-acceptance-niri` and `just test-acceptance-cosmic` locally, in the scope 1.3 decided for cosmic. Verify: green, except documented skips.

## 7. Documentation

- [ ] 7.1 Update the docs:
  - `dev-docs/platform-linux-wayland.md`: §3 the GTK 3 shadow limit under niri and cosmic; §6 the implemented backends; §10 the support matrix; §15 the roadmap.
  - `dev-docs/architecture.md` §10: the platform snapshot.
  - `CONTRIBUTING.md`: how to run the local lanes and which packages they need.

  Verify by comparing each statement with the specs.

## 8. Verification

- [ ] 8.1 Run `just check`, `just test` and `just test-python`; all must be green.
- [ ] 8.2 Run `just headless=true test-acceptance-compositor`, `test-acceptance-x11` and `test-acceptance-x11-bare`, and confirm that the existing lanes are unchanged.
- [ ] 8.3 Confirm that the local niri and cosmic lanes from 6.4 are green on the final state.

## Workflow follow-up

- Commit and push only on the maintainer's explicit go-ahead.
- Archive after the local lanes and CI are green.
