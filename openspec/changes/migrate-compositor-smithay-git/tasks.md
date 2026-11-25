# Tasks

## 1. Characterization tests against smithay 0.7 (Phase 0)

- [ ] 1.1 Add the dev-dependency features the raw test clients need to `apps/wayland-compositor/Cargo.toml`:
  - `wayland-protocols` with `client` and `staging`
  - `wayland-protocols-wlr` with `client`
  - `wayland-protocols-misc` with `client`

  Verify that `cargo tree --workspace -e dev -i wayland-protocols-wlr` shows no new major version.
- [ ] 1.2 Create `apps/wayland-compositor/tests/shared/client.rs`:
  - Move the toplevel and `wl_shm` buffer code out of `tests/shared/popup_client.rs` into it, and add a solid-color toplevel of a given size.
  - Add a headless start helper with a unique socket name, the control socket enabled, and switches for `--no-eis`, `--restrict-protocols`, `--scale`, `--outputs` and `--print-env`.
  - Add `get_pointer_position`, `get_modifiers` and `list_windows` wrappers.

  Verify that `ipc_tests` and `pidns_tests` still compile, and that `just test-crate platynui-wayland-compositor` stays green.
- [ ] 1.3 Add `tests/protocol_policy_tests.rs` with the four registry scenarios: "Restricted clients see no virtual-input globals", "Unrestricted clients see them", "Restricted clients cannot find screen capture" and "Unrestricted clients find it". Verify that they pass on smithay 0.7.
- [ ] 1.4 Add `tests/input_emulation_tests.rs` with the virtual-pointer scenarios:
  - "A virtual-pointer click activates the window under it"
  - "Relative motion past the right edge stops at the edge"
  - "An output-bound virtual pointer reaches that output's center", with `--outputs 2`
  - "Zero extent is ignored"

  Verify that they pass on smithay 0.7.
- [ ] 1.5 Add the virtual-keyboard scenarios to `tests/input_emulation_tests.rs`: "A custom virtual-keyboard keymap is honored" and "A destroyed virtual keyboard clears its modifiers". The focused raw client records the `wl_keyboard.keymap` and `key` events it receives. Verify that both pass on smithay 0.7.
- [ ] 1.6 Add the EIS scenarios to `tests/input_emulation_tests.rs`, using a `reis` EI client:
  - "The child process finds the EIS socket"
  - "EIS can be switched off"
  - "A receiver context is refused"
  - "Binding every capability yields every device except text"
  - "The EI keymap matches the compositor keymap", started with a non-default `--keyboard-layout`
  - "A key typed through EIS reaches the focused client"
  - "An EIS click activates the window under it"
  - "EIS absolute motion lands on the requested global position"

  Verify that all pass on smithay 0.7.
- [ ] 1.7 Add the scenario "A warp inside the focused surface moves the pointer" to `tests/input_emulation_tests.rs`. Verify that it passes on smithay 0.7.
- [ ] 1.8 Add `tests/screen_capture_tests.rs` with these scenarios:
  - "A full-output capture succeeds"
  - "A scaled output announces its mode size"
  - "A covered toplevel is captured without what covers it"
  - "A wrong-sized buffer is rejected and the session recovers"
  - "The cursor appears only when asked for"
  - "A cursor session reports the cursor image and hotspot"

  Verify that all pass on smithay 0.7.
- [ ] 1.9 Record the baseline:
  - Save the global/version listing of a headless compositor on smithay 0.7 (`wayland-info`, or a small registry dump from the test client) to the scratch area for the comparison in 7.2.
  - Run `just test-acceptance-compositor --exitonfailure` once to confirm the lane is green before the migration.

## 2. Pin smithay and adapt the compositor (Phase 1)

- [ ] 2.1 In `apps/wayland-compositor/Cargo.toml`:
  - Replace the smithay dependency with `git = "https://github.com/Smithay/smithay.git", rev = "54cf9c10fe214066f2e69f43784aafde0d921682"`. Keep the feature list, and add a comment naming the revision date and pointing to `CONTRIBUTING.md` §7.
  - Change `winit` to `version = "0.31.0-beta.4"` with the same features.

  Let Cargo update `Cargo.lock`. Verify that `cargo tree --workspace -i winit@0.30.13` lists only the eframe/egui consumers and not `platynui-wayland-compositor`.
- [ ] 2.2 Apply the mechanical API changes (design D4, step 1):
  - one `delegate_dispatch2!(State)` instead of the `delegate_*!` macros in `state.rs`, `xwayland.rs` and `handlers/foreign_toplevel.rs`
  - the touch `TouchTarget`/`TouchGrab` signatures in `focus.rs` and `grabs.rs`
  - `InputTime`, `FrameMarker` and `last_frame`
  - the `*_state()` accessor mutability in `handlers/{selection,data_control}.rs`
  - `XdgDecorationHandler::decoration_destroyed` in `handlers/decoration.rs`
  - removal of `XdgDialogHandler::modal_changed` in `handlers/xdg_dialog.rs`
  - `with_committed_state()` where `current_state()` was used
  - the `TabletSeatHandler` import in `handlers/cursor_shape.rs`

  Verify that `cargo check -p platynui-wayland-compositor` reports only the DnD, virtual-keyboard and winit errors.
- [ ] 2.3 Implement the DnD rework (D4, step 2):
  - `DndGrabHandler`
  - `WaylandDndGrabHandler::started`, which starts a `DnDGrab` for the seat as anvil does
  - `DndFocus` for `PointerFocusTarget`, as `X11Wm::start_wm` requires

  Verify that `cargo check -p platynui-wayland-compositor` no longer reports selection or Xwayland errors.
- [ ] 2.4 Implement `VirtualKeyboardHandler` per D5:
  - one keymap helper that switches between the compositor keymap and a device `KeymapFile` only when the key source changes, called before `KeyboardHandle::input` in `src/input.rs`, `src/control.rs` and `src/eis.rs`
  - routing through `process_input_event`
  - on `DeviceRemoved`, releasing that device's keys and clearing the modifier bits it set

  Verify that the 1.5 tests pass unchanged.
- [ ] 2.5 Port `src/backend/winit.rs` to winit 0.31.0-beta.4 (`Icon`, `CursorIcon`, `WindowAttributesExtWayland`). Verify that `cargo build -p platynui-wayland-compositor` succeeds, and that `scripts/startcompositor.sh --backend nested` opens a window that accepts pointer and keyboard input.
- [ ] 2.6 Correct the module comments in `src/handlers/toplevel_icon.rs` and `src/handlers/toplevel_tag.rs`: smithay provides `wayland::xdg_toplevel_icon` / `wayland::xdg_toplevel_tag`, and these handlers stay our own for now. Verify that `grep -rn "Smithay 0.7 does not" apps/wayland-compositor/src/handlers/toplevel_{icon,tag}.rs` finds nothing.
- [ ] 2.7 Document the pin:
  - In `CONTRIBUTING.md` §7, add one rule for git-pinned Rust dependencies: a full `rev` and never a branch; moved only in its own change and verified by the crate's tests and the lane that uses it.
  - In `dev-docs/platform-linux-wayland.md` §11.7, state that the compositor uses a pinned smithay revision.

  Verify both by reading the rendered Markdown.
- [ ] 2.8 Verify Phase 1:
  - `just check` is clean.
  - `just test-crate platynui-wayland-compositor` passes, with every Phase 0 test unchanged.
  - `just test-acceptance-compositor --exitonfailure` is green.
  - Manual Xwayland smoke check: start `scripts/startcompositor.sh --xwayland`, then run an X11 GTK application (`GDK_BACKEND=x11`) and the Swing test app. Check that the `list_windows` rectangles match the visible frames (D-risk `_GTK_FRAME_EXTENTS`), that `focus_window` activates each window, and that typing right after a window maps reaches it.

## 3. Screen capture on smithay's protocol code (Phase 2a)

- [ ] 3.1 Add the scenarios "A second pending frame is a protocol error" and "Closing the captured toplevel stops its session" to `tests/screen_capture_tests.rs`. Run them on the current code and note in the test's commit message which ones fail.
- [ ] 3.2 Replace the hand-written protocol code in `src/handlers/screencopy.rs` with smithay's `ImageCopyCaptureHandler`, `OutputCaptureSourceHandler` and `ToplevelCaptureSourceHandler`, each with `new_with_filter(!restrict)`, per D7:
  - constraints: shm ARGB8888/XRGB8888, no dmabuf
  - our offscreen rendering and shm copy are called from `frame()` and `cursor_frame()`
  - toplevel handles resolve through the ext-foreign-toplevel bookkeeping
  - sessions are stopped when their source goes away

  Verify that every test in `screen_capture_tests.rs` and `protocol_policy_tests.rs` passes, and that `ipc_tests::ipc_screenshot` stays green.

## 4. Virtual pointer on smithay (Phase 2b)

- [ ] 4.1 Extract `move_pointer_to(state, global_pos, time)` from `handle_pointer_motion_absolute` (`src/input.rs:143`) per D6, and keep the winit path's own computation in front of it. Verify that `just test-crate platynui-wayland-compositor` stays green.
- [ ] 4.2 Replace `src/handlers/virtual_pointer.rs` with `VirtualPointerManagerState::new(dh, !restrict)` and a `VirtualPointerHandler`:
  - events go into `process_input_event`
  - absolute motion is mapped through the device's output, or the combined geometry, into `move_pointer_to`
  - a destroyed device releases its buttons

  Verify that the 1.4 tests and the virtual-input registry tests pass.

## 5. Pointer warp on smithay (Phase 2c)

- [ ] 5.1 Add these scenarios to `tests/input_emulation_tests.rs`:
  - "A warp with a stale serial is ignored"
  - "A warp outside the surface or to an unfocused surface is ignored"
  - a warp inside a mapped `xdg_popup` (using the popup client) that lands at the popup's global position

  Confirm that they fail on the current code.
- [ ] 5.2 Replace `src/handlers/pointer_warp.rs` per D8:
  - a filtered `GlobalDispatch2` global that hands the resource to `smithay::wayland::GlobalData`
  - `PointerWarpHandler::warp_pointer`, which checks focus (including an implicit grab), the serial against `PointerHandle::last_enter()`, and the surface bounds, converts to global coordinates for root, subsurface and popup surfaces, and moves the pointer through `move_pointer_to`

  Verify that all pointer-warp tests and the registry tests pass.

## 6. EIS on smithay's libei backend (Phase 2d)

- [ ] 6.1 Add these scenarios to `tests/input_emulation_tests.rs`:
  - "A second EI client does not displace the first"
  - "A crashed EI client leaves no key pressed"
  - "Another device's held key survives"

  Confirm that they fail on the current code.
- [ ] 6.2 Keep owned copies of the resolved XKB values (rules, model, layout, variant, options) in `State`, so an `XkbConfig` can be built after startup (D9). Verify with a unit test that the config built from `State` compiles to the same keymap string as the seat keyboard's.
- [ ] 6.3 Replace `src/eis.rs` per D9:
  - listener on `$XDG_RUNTIME_DIR/eis-platynui` with the `LIBEI_SOCKET` export unchanged
  - one `EiInput` source per connection
  - seat `default` with keyboard (owned XKB values), pointer, absolute pointer and touch, each with per-output `EiRegion`s, and no text
  - `update_regions` when outputs change
  - events into `process_input_event`, with absolute coordinates through `move_pointer_to`
  - per-connection tracking of held keys and buttons, released on `Disconnected`
  - remove the `eis_client_*` fields from `State`

  Verify that every EIS test in `input_emulation_tests.rs` passes.
- [ ] 6.4 Check PlatynUI's EIS client and the standalone client against the new server. Run `platynui-eis-test-client probe`, `click`, `type-text "Hallo ä"` and `tap` against a compositor started with `scripts/startcompositor.sh`. Verify that each command takes effect in the egui test app, and that `just test-crate platynui-platform-linux-wayland` stays green.
- [ ] 6.5 In `dev-docs/platform-linux-wayland.md` §11.7, describe the EIS server as smithay's libei backend serving several clients at once. Verify by reading the rendered Markdown.

## 7. Integration checks

- [ ] 7.1 Run the full local gate on the final state:
  - `just check`
  - `just test`
  - `just test-acceptance-compositor --exitonfailure`
  - the manual Xwayland smoke check from 2.8

  All must be green.
- [ ] 7.2 Compare the global/version listing with the 1.9 baseline. Every global from the baseline must still be advertised. Version changes must be the expected upstream bumps (for example `xdg_wm_base` 7, `zxdg_decoration_manager_v1` 2, `zwlr_layer_shell_v1` 5). Record the comparison in the PR description.
- [ ] 7.3 After pushing (only when the maintainer asks for it), confirm that the CI run is green in every job, including the Windows, macOS and Linux wheel jobs, which now fetch the smithay git source.

## Workflow follow-up

- Revise `fix-compositor-xwayland-startup-hang` so it kills a hung Xwayland through the pidfd that smithay now exposes. That change stays outside this one.
- Optionally propose `PointerWarpManager::new_with_filter` upstream; once merged, a later pin move can drop the filtered global type (design D8).
- Archive this change after review.
