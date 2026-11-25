# Design

## Context

See proposal.md for the motivation and the scope of both phases. This section covers only what shapes the approach. Unless stated otherwise, references are to the working tree at `cb5611fc` and to smithay `master` at `54cf9c10`.

Verified facts:

- **One consumer.** smithay is a dependency of `apps/wayland-compositor` only (`apps/wayland-compositor/Cargo.toml:15`); no other workspace member uses it. The direct `winit = "0.30"` dependency (`Cargo.toml:46`) exists to enable `wayland-csd-adwaita` for smithay's winit backend.
- **Trial build.** A standalone copy of the compositor was built against `54cf9c10`. With winit still at 0.30, winit fails to compile: without platform features it hits winit's `compile_error!`, because smithay no longer enables those features on 0.30. Once winit moves to 0.31, rustc reports 40 errors in 11 files: `state.rs`, `xwayland.rs`, `focus.rs`, `grabs.rs`, `backend/winit.rs` and `handlers/{selection,data_control,decoration,xdg_dialog,cursor_shape,foreign_toplevel}.rs`. That is only the first wave; rustc stops before it checks function bodies.
- **winit versions.** Smithay moved to winit 0.31 betas on 2026-06-11 (`f949fa48`) and has pinned `=0.31.0-beta.4` since 2026-10-27 (`905ecda4`). eframe 0.37 and egui `master` still require winit 0.30.13, and egui's port to 0.31 is an open draft (emilk/egui#7731). With winit 0.31 the trial build no longer contains winit 0.30 at all.
- **The handlers to replace are untested.** `screencopy.rs`, `virtual_pointer.rs`, `pointer_warp.rs`, `eis.rs` and `virtual_keyboard.rs` contain no `#[test]`, and the integration tests (`tests/ipc_tests.rs`, `child_exit_tests.rs`, `pidns_tests.rs`) do not touch these protocols. `child_exit_tests.rs:32` even starts the compositor with `--no-eis`.
- **How PlatynUI injects input here.** It uses the control socket first, then EIS, then the portal (`crates/platform-linux-wayland/src/input/mod.rs:154`), and takes screenshots through the control socket (`crates/platform-linux-wayland/src/screenshot.rs:41`). Virtual keyboard, virtual pointer, pointer warp and screencopy therefore serve only external tools.
- **Generic input path.** `input::process_input_event<B: InputBackend>` (`src/input.rs:39`) already handles any smithay input backend. EIS and virtual-pointer buttons already go through `input::process_pointer_button` (`src/eis.rs:304`, `src/handlers/virtual_pointer.rs:272`). Keys reach `KeyboardHandle::input` in three places: devices (`src/input.rs:105`), the control socket (`src/control.rs:827`) and EIS (`src/eis.rs:362`).
- **Restrictive policy.** Under `--restrict-protocols` the virtual-keyboard, virtual-pointer, pointer-warp, output-management and screencopy globals are hidden from every client (`src/state.rs:416-434`).
- **The compositor keymap is static.** It is built from CLI and environment values by `resolve_xkb_config` (`src/lib.rs:256`), and the control socket has no command to change it.
- **Xwayland is not exercised by the acceptance lane.** `real-wayland` runs `scripts/startcompositor.sh` without `--xwayland` (`robot.toml:82-86`).
- **Smithay `master` APIs:**
  - Most of the new modules take a global filter: `VirtualPointerManagerState::new(dh, filter)`, `OutputCaptureSourceState::new_with_filter`, `ToplevelCaptureSourceState::new_with_filter` and `ImageCopyCaptureState::new_with_filter`.
  - `PointerWarpManager::new(dh)` takes no filter. Its requests are dispatched through the public unit type `smithay::wayland::GlobalData` (`src/wayland/mod.rs:105` in smithay).
  - `backend::libei` disconnects receiver contexts itself (`src/backend/libei/mod.rs:192`).
  - `EiInputSeat::add_keyboard` compiles its keymap from an `XkbConfig` (`src/backend/libei/seat.rs:80`).
  - EI absolute events return global coordinates from `x_transformed`/`y_transformed` and ignore the size argument (`src/backend/libei/input.rs:217-257`).
- **License.** smithay and its `anvil` example are MIT, so they may serve as reference code. niri is `GPL-3.0-or-later`: read it to understand an approach, never copy from it.

Assumed, not verified:

- Cargo fetches the smithay git source on every platform, because it resolves the whole workspace. `Swatinem/rust-cache` keeps `~/.cargo/git` between CI runs.
- The headless backend renders the colors and cursor that the capture tests check, as it does for `ipc_screenshot` (`tests/ipc_tests.rs:364`).

## Goals / Non-Goals

**Goals:**

- Phase 1 leaves every observable behavior unchanged except the upstream behavior listed in the proposal. The characterization tests written before the migration stay green without edits.
- Every handler that Phase 2 replaces is under test before it is replaced, and each replacement is a separate commit that can be reverted on its own.
- Moving the pin later is routine work: one manifest line plus the API fixes that upstream requires.

**Non-Goals:**

- New capture features: dmabuf capture, damage tracking beyond what is sent today, capture of layer surfaces.
- The EIS text capability (`ei_text`) and IME work.
- Adopting smithay's `xdg_toplevel_icon` / `xdg_toplevel_tag` modules. This change only corrects the comments. Adopting them would change titlebar icon handling, which is out of scope.
- Making the ext-foreign-toplevel identifier our stable window id. Smithay now allows it (`60aafc2b`); that is a separate change.
- Killing a hung Xwayland. That belongs to `fix-compositor-xwayland-startup-hang`, which can then use the pidfd.
- Removing the second winit version from the workspace. That happens when egui moves to winit 0.31.

## Decisions

### D1 — Pin a full git revision in the compositor's manifest

`smithay = { git = "https://github.com/Smithay/smithay.git", rev = "54cf9c10fe214066f2e69f43784aafde0d921682", default-features = false, features = [<unchanged>] }`, with a comment that names the date of the revision and points to the update rule in `CONTRIBUTING.md` §7.

The rule: the pin moves only in a change of its own. That change runs the compositor tests and the `real-wayland` lane, and is reverted as a whole if a regression shows.

Alternatives:
- **`branch = "master"`** moves on every `cargo update`. That makes builds irreproducible and pulls upstream regressions in unannounced (niri's update log names several).
- **`[patch.crates-io]`** hides the real source and still needs a version that matches 0.7.
- **A fork** costs maintenance that nobody needs.
- **`[workspace.dependencies]`** offers nothing for a single consumer.

### D2 — Accept winit 0.31.0-beta.4 for the compositor

The compositor's own dependency becomes `winit = { version = "0.31.0-beta.4", default-features = false, features = ["wayland-csd-adwaita"] }`. Written as a caret requirement, it lets smithay's exact `=` pin choose the beta on later moves. API churn between betas stays inside `src/backend/winit.rs`.

Alternatives:
- **Pin before 2026-06-11** (`5627819c`, the last smithay revision on winit 0.30). That keeps 322 of the 539 commits. It loses what this change is for: the Xwayland pidfd, inherited-environment spawn, modal dialogs, the WM_STATE/withdraw/unmap-loop fixes, the October fixes to how the active window is tracked, smithay's virtual pointer and libei backend, the late `wl_pointer` enter and the ClickGrab fixes. The large API changes (`Dispatch2`, the DnD rework, `last_acked`) land before that date anyway.
- **Disable smithay's `backend_winit`** and drop the nested mode.
- **A fork that reverts the bump.**

### D3 — Characterization tests first, against smithay 0.7

New integration tests under `apps/wayland-compositor/tests/`:
- `input_emulation_tests.rs`
- `screen_capture_tests.rs`
- a registry test for the restrictive policy

They start the compositor headless, as `ipc_tests.rs` does, and observe through the control socket (`get_pointer_position`, `get_modifiers`, `list_windows`) and through raw clients.

The raw clients are built on the existing dev-dependencies `wayland-client` and `wayland-protocols`:
- add the `staging` feature for image-copy-capture and pointer-warp, the `wlr` client protocols for the virtual pointer, and `misc` for `zwp_virtual_keyboard_v1`
- the toplevel and buffer code is shared with `tests/shared/popup_client.rs`, moved into a new `tests/shared/` module

The EI client uses `reis`, which the compositor already depends on.

Tests for unchanged behavior are written in Phase 0 and run against 0.7. Tests for deliberate behavior changes are written at the start of the Phase 2 step that introduces the change:
- pointer warp focus, serial and bounds
- release on EI disconnect
- several EI clients

Alternative: rely on the acceptance lane. It never exercises these protocols (see Context).

### D4 — Phase 1 order: mechanical first, semantic second, winit last

1. **Mechanical changes:**
   - `delegate_*!` → one `delegate_dispatch2!(State)`
   - touch target/grab signatures, `InputTime`, `FrameMarker`
   - `with_committed_state()`
   - `XdgDecorationHandler::decoration_destroyed` returns the toplevel to client-side decorations, the same effect as unsetting the mode
   - drop `XdgDialogHandler::modal_changed`
   - mutability of the `*_state()` accessors
   - the moved `TabletSeatHandler`
2. **Semantic changes:** the virtual keyboard (D5) and the DnD rework. For DnD, implement `DndGrabHandler`, then `WaylandDndGrabHandler::started` to start a `DnDGrab` as anvil does, then `DndFocus` for `PointerFocusTarget` (`src/focus.rs:47`), which `X11Wm::start_wm` now requires.
3. **winit 0.31** in `src/backend/winit.rs` (`Icon`, `CursorIcon`, `WindowAttributesExtWayland` moved).

anvil (MIT) is the reference for every step.

### D5 — Virtual keyboard: activate the device keymap per source, release on removal

Smithay now delivers virtual-keyboard input as `InputEvent<VirtualKeyboardBackend>` through `VirtualKeyboardHandler`, and leaves the keymap and modifier bookkeeping to the compositor.

**Keymap activation.** One helper owns "which keymap is active on the seat keyboard": the compositor's (built once from `resolve_xkb_config`) or a virtual keyboard's `KeymapFile`.
- Before every key from any source it switches only when the source changes (`set_keymap_from_keymap_file`). The three key entry points from Context call it before `KeyboardHandle::input`.
- Switching only on a change of source keeps the number of `wl_keyboard.keymap` events a client receives small.

**Removal.** On `DeviceRemoved` the compositor releases the keys that device holds, using the per-device `KeyboardSource` that smithay now tracks. It then clears the modifier bits that device set through `modifiers`, tracked per device, as smithay's changelog requires.

**Reference.** anvil's `VirtualKeyboardHandler` (`anvil/src/input_handler.rs:1404`).

**Alternative:** keep the compositor keymap for virtual keyboards. That breaks `wtype`-style clients that upload their own keymap, which smithay 0.7 supported.

### D6 — One absolute-motion helper for emulated pointers

Extract the body of `handle_pointer_motion_absolute` (`src/input.rs:143`) into `move_pointer_to(state, global_pos, time)`:
- the winit path keeps computing `position_transformed` over the combined geometry and calling it
- the virtual pointer computes the position from its device's output (`VirtualPointerDevice::output()`), or from the combined geometry when it has none
- EIS passes its global coordinates through unchanged

The generic path would add the layout origin on top of global EI coordinates, and it cannot see a virtual pointer's output.

**Alternative:** special-case these backends inside the generic handler with `TypeId` checks. That is less explicit and easy to break with the next backend.

### D7 — Screen capture on smithay's state machine, our rendering

The protocol objects move to smithay:
- `ImageCopyCaptureHandler`, `OutputCaptureSourceHandler` and `ToplevelCaptureSourceHandler`, each created with `new_with_filter(!restrict)`
- constraints of `shm: [Argb8888, Xrgb8888]`, `dma: None`, and the size today's `source_buffer_size` computes (`src/handlers/screencopy.rs:667`)

These parts stay ours and are called from `frame()` and `cursor_frame()`:
- rendering into an offscreen texture (`render_source_to_pixels`, `render_cursor_surface_to_pixels`)
- the shm copy (`copy_pixels_to_shm`, `copy_cursor_to_shm`)
- the metadata

A toplevel source resolves its `ForeignToplevelHandle` to our `Window` through the existing ext-foreign-toplevel bookkeeping in `src/handlers/foreign_toplevel.rs`.

The compositor keeps the live sessions for each source. It stops them (`Session::stop`) when the toplevel unmaps or the output is removed, which `stopped` in the spec requires.

**Alternative:** keep the hand-written protocol code. That keeps 1147 lines whose protocol corner cases smithay now covers (`duplicate_frame`, inert destruction order).

### D8 — Pointer warp: smithay's request handling behind our filtered global

`PointerWarpManager` offers no filter. The compositor therefore creates the `wp_pointer_warp_v1` global itself:
- with a small `GlobalDispatch2` type whose `can_view` applies the policy
- whose `bind` initializes the resource with `smithay::wayland::GlobalData`

Requests then go to smithay's `Dispatch2` implementation, which calls `PointerWarpHandler::warp_pointer`.

The handler honors a warp only under three conditions:
1. the pointer focus is on that surface, or on its root during an implicit grab
2. the serial equals `PointerHandle::last_enter()` for that client
3. the position lies inside the surface's bounding box

It converts the position to global coordinates from the surface's root window location plus the offset of the subsurface or popup, then moves the pointer through `move_pointer_to`.

**Alternatives:**
- Advertise pointer warp unfiltered. That changes policy behavior without a reason.
- Contribute `new_with_filter` upstream first. Worth doing, but the change does not wait for it. Once it is merged, the compositor switches to it on the next pin move.

### D9 — EIS on smithay's libei backend

The listener keeps the socket `$XDG_RUNTIME_DIR/eis-platynui` and the `LIBEI_SOCKET` export (`src/eis.rs:27`, `src/environment.rs:56`). Each accepted context gets its own `EiInput` source, so several clients are served at once.

On `Connected` the compositor adds a seat `default` with:
- a keyboard built from the same `XkbConfig` values as the seat keyboard. `State` keeps owned copies of the resolved fields, because `resolve_xkb_config` borrows the CLI args.
- a relative pointer
- an absolute pointer and touch, each with one `EiRegion` per output

It never adds text. Regions are updated with `update_regions` when outputs change.

`EiInputEvent::Event` goes into `process_input_event`. Absolute motion uses D6, and keys go through the keymap helper from D5 with the compositor keymap.

The compositor tracks the keys and buttons each connection holds. On `Disconnected` it releases them through the normal path. It also drops the `eis_client_*` fields from `State` (`src/state.rs:151-153`).

**Alternative:** keep `eis.rs`. It duplicates event decoding that smithay maintains, and it is single-client by construction.

### D10 — Documentation of the pin

- `CONTRIBUTING.md` §7 gains one rule for git-pinned crates: pin a full `rev`, never a branch; move it only in its own change, verified by the crate's tests and the lane that uses it.
- `dev-docs/platform-linux-wayland.md` §11.7 states that the compositor uses a pinned smithay revision, and that EIS serves several clients through smithay's backend.
- The false "smithay 0.7 has no abstraction" comments are corrected: `toplevel_icon.rs:9` and `toplevel_tag.rs:14` are already wrong, and the comments in the replaced files go away with them.

## Risks / Trade-offs

- **[Risk] An upstream regression at the pinned revision.** → The characterization tests and the lane run before the pin is accepted. Moving forward or back is a one-line change, and Phase 1 can be reverted as one commit.
- **[Risk] The ack-before-commit enforcement disconnects a fixture or a real application with a protocol error.** → The lane covers the egui, Qt and GTK fixtures. A client that fails is a client bug to report; the compositor gets no workaround that weakens the protocol.
- **[Risk] `_GTK_FRAME_EXTENTS` changes the geometry of X11 GTK windows and interacts with our titlebar offset (`src/xwayland.rs:192`, `:353`).** → A manual Xwayland smoke check with an X11 GTK application and a Swing application. Window rectangles from `list_windows` are compared with the visible frame.
- **[Risk] winit beta churn on later pin moves.** → It is confined to `src/backend/winit.rs`. The nested mode is a development convenience, and CI uses headless.
- **[Trade-off] The workspace compiles winit 0.30 and 0.31** (with their smithay-client-toolkit stacks) until egui moves to winit 0.31. Each binary links one version, so the cost is build time only, not binary size or runtime.
- **[Risk] Keymap switching for virtual keyboards sends extra `wl_keyboard.keymap` events.** → Switch only when the input source changes (D5). The virtual-keyboard test checks that a control-socket key after a virtual-keyboard key arrives with the compositor keymap.
- **[Risk] Cargo now clones a git source for every workspace build, including Windows and macOS wheel jobs.** → It is a one-time fetch per fresh cache. The first CI run after the change confirms that all wheel jobs stay green.
- **[Risk] Copying from GPL code.** → Only anvil (MIT) is used as reference code. niri is consulted for understanding only.

## Migration Plan

- **Phase 0, additive:** characterization tests against smithay 0.7. No behavior change, no native rebuild.
- **Phase 1, behavioral only through upstream:** the pin, the winit bump and the API adaptation in one commit. The Phase 0 tests stay green unchanged, plus `just test-crate platynui-wayland-compositor`, the `real-wayland` lane and the manual Xwayland smoke check.
- **Phase 2, behavioral:** one commit per replaced handler, in this order: screencopy, virtual pointer, pointer warp, EIS. Each commit first adds the tests for its deliberate behavior changes, then replaces the handler. EIS goes last because it is PlatynUI's fallback input path.
- **No native rebuild.** `packages/native` does not depend on the compositor, and the Python package and Robot Framework library are unchanged. The lane uses the compositor binary that `scripts/startcompositor.sh` builds.
- **Rollback:** revert the Phase 2 commits in reverse order, then the Phase 1 commit, which restores `smithay = "0.7"`, `winit = "0.30"` and the old `Cargo.lock`. The Phase 0 tests stay, because they describe behavior that holds on both versions.

## Open Questions

- Should the pointer-warp filter be proposed upstream as `PointerWarpManager::new_with_filter`? This does not change the approach (D8). It only decides whether a later pin move can drop our small global type.
