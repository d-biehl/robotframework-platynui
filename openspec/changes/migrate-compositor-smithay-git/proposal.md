# Proposal

## Why

The PlatynUI Wayland compositor depends on smithay 0.7.0. That is still the newest crates.io release, published on 2025-06-24. Smithay's `master` has gained 539 commits since then, and no 0.8 release is planned (there is no milestone or release issue for one). Smithay-based compositors such as niri therefore pin a git revision and move it forward deliberately.

Staying on 0.7.0 costs us in three ways:

- **We maintain protocol code that smithay now provides.** These modules are hand-written today:
  - `ext-image-copy-capture-v1` (`src/handlers/screencopy.rs`, 1147 lines)
  - `zwlr-virtual-pointer-v1` (`src/handlers/virtual_pointer.rs`, 425 lines)
  - `wp-pointer-warp-v1` (`src/handlers/pointer_warp.rs`, 120 lines)
  - the EIS server (`src/eis.rs`, 428 lines)

  None of them is covered by a test.
- **We miss upstream fixes that matter for automation:**
  - X11 activation through `_NET_ACTIVE_WINDOW` and the later corrections to how the active window is tracked.
  - Modal X11 dialogs.
  - Keys typed right after an X11 window maps are no longer lost.
  - The `WM_STATE`, iconic-map, ICCCM-withdraw and endless-unmap-loop fixes.
  - A client that creates its `wl_pointer` late now receives `enter`.
  - Keyboard focus returns after a popup is dismissed.
  - `ClickGrab` follows a moving target.
  - Frame-callback throttling fixes.
- **The planned `fix-compositor-xwayland-startup-hang` change has to leave a gap.** It cannot kill a hung Xwayland, because smithay 0.7 exposes no process handle. `master` exposes the Xwayland pidfd and can also start Xwayland with the full environment.

## What Changes

The change runs in two phases. Each phase can be committed on its own and is verified on its own.

**Phase 1: migrate to a pinned smithay revision, without intended behavior changes of our own**

- Replace `smithay = { version = "0.7", … }` with a git dependency on `https://github.com/Smithay/smithay.git` at revision `54cf9c10fe214066f2e69f43784aafde0d921682` (smithay `master` of 2026-11-24). Move the pin only deliberately; every move is verified like a migration.
- Move the compositor's own `winit` dependency (it exists to enable the Adwaita CSD feature) from `0.30` to `0.31.0-beta.4`, the version smithay pins exactly.
  - eframe 0.37 still requires winit 0.30, so the workspace then builds both versions.
  - Each binary links only one: the compositor 0.31, the Inspector and `test-app-egui` 0.30.
- Adapt the compositor to smithay's API changes:
  - `delegate_*!` macros → `delegate_dispatch2!`
  - touch grab/target signatures, `InputTime`, `FrameMarker`
  - the Drag-and-Drop rework (`WaylandDndGrabHandler`, `DndFocus` for the X11 window manager)
  - `XdgDecorationHandler::decoration_destroyed`; removal of `XdgDialogHandler::modal_changed`; `with_committed_state()`
  - the virtual-keyboard rework: a virtual keyboard is now a regular input device, and the compositor activates its keymap and reconciles modifiers itself
  - winit 0.31 in `src/backend/winit.rs`
- Behavior adopted from upstream, not chosen by us:
  - **BREAKING for nonconforming clients:** xdg-shell and layer-shell now reject a buffer commit before the client acknowledges a configure, with a protocol error.
  - For X11 windows that set `_GTK_FRAME_EXTENTS`, the window geometry excludes the client-side shadow.
  - The Xwayland window manager fixes listed under Why.

**Phase 2: replace hand-written protocol handlers with smithay modules**

- **Screencopy** moves to `wayland::image_capture_source` + `wayland::image_copy_capture`. Output and toplevel capture, `wl_shm` ARGB8888/XRGB8888, `paint_cursors` and cursor sessions stay as they are, and so does hiding the globals under `--restrict-protocols`. No dmabuf capture is added.
- **Virtual pointer** moves to `wayland::virtual_pointer`. Events enter the compositor's generic input path (`input::process_input_event`), and absolute motion of an output-bound pointer still maps onto that output.
- **Pointer warp** moves to `wayland::pointer_warp`. **Behavior change:** a warp is honored only for a surface that has pointer focus, with a valid enter serial, and to a position inside that surface. Today the first matching toplevel surface is accepted and the serial is ignored.
- **EIS** moves to `backend::libei`:
  - Unchanged: the socket name `eis-platynui`, the `LIBEI_SOCKET` export, sender-only contexts, the compositor's configured keymap on the EI keyboard, regions for absolute pointer and touch, and no text capability.
  - EI events enter the generic input path.
  - **Behavior change:** when an EI client disconnects, the keys and buttons it still holds are released. Today they stay pressed until another device releases them.
  - **Behavior change:** several EI clients are served at once, each with its own devices. Today a second connection replaces the first.
- **Toplevel icon and tag:** correct the false "smithay 0.7 has no abstraction" claims in `src/handlers/toplevel_icon.rs` and `src/handlers/toplevel_tag.rs`. Smithay ships `wayland::xdg_toplevel_icon` and `wayland::xdg_toplevel_tag`, and has since 0.7.0.
- **Characterization tests come first** for every replaced handler and for the virtual keyboard. They are written against the current behavior and must stay green through both phases.

## Capabilities

### New Capabilities

- `compositor-input-emulation`: how the PlatynUI compositor accepts emulated input from protocol clients — EIS, `zwlr-virtual-pointer-v1`, `zwp-virtual-keyboard-v1`, `wp-pointer-warp-v1`. That covers where it is processed, which keymap and coordinate space applies, what happens when a device goes away, and how the restrictive protocol policy gates it.
- `compositor-screen-capture`: what the PlatynUI compositor serves through `ext-image-copy-capture-v1` and `ext-image-capture-source-v1`: output and toplevel sources, buffer formats, cursor painting and cursor sessions, and gating under the restrictive policy.

### Modified Capabilities

None. `compositor-modifier-state` already requires the modifier state to reflect EIS and the virtual-input protocols, and it stays satisfied.

## Impact

- **Rust, compositor (`apps/wayland-compositor`):**
  - `Cargo.toml` and the workspace `Cargo.lock`. The lock moves calloop to 0.14.5, the wayland-rs crates and x11rb to 0.14 for smithay, and adds winit 0.31.0-beta.4.
  - Phase 1 touches these files: `state.rs`, `xwayland.rs`, `focus.rs`, `grabs.rs`, `handlers/{selection,data_control,decoration,xdg_dialog,cursor_shape,foreign_toplevel,virtual_keyboard}.rs`, `backend/winit.rs`.
  - Phase 2 replaces `handlers/{screencopy,virtual_pointer,pointer_warp}.rs` and `eis.rs`, and touches `input.rs` and `backend/mod.rs`.
- **Other Rust crates:** none change. `platform-linux-wayland` reaches the compositor through the control socket first, then EIS, then the portal (`crates/platform-linux-wayland/src/input/mod.rs:154`). EIS keeps its socket and its handshake, so the platform's EIS client is unaffected.
- **Python / Robot Framework:** no code change and no native rebuild (`packages/native` does not depend on the compositor). The acceptance suites under `tests/` exercise the compositor through the `real-wayland` profile.
- **Tests:**
  - New integration tests in `apps/wayland-compositor/tests/`: screencopy, virtual pointer, virtual keyboard, pointer warp and EIS. They use raw Wayland clients modeled on `tests/shared/popup_client.rs` and a `reis` EI client.
  - Existing tests that must stay green: `ipc_tests`, `child_exit_tests`, and the `#[ignore]`d `pidns_tests`.
  - The acceptance lane `just test-acceptance-compositor` (profile `real-wayland`). It starts the compositor without Xwayland and the X11 lanes do not use the compositor, so Xwayland gets a manual smoke check through `scripts/startcompositor.sh --xwayland`.
- **Platforms:** Linux only. The compositor's dependencies are `cfg(target_os = "linux")`. Cargo still has to fetch the smithay git source to resolve the workspace on every platform, which adds a one-time clone in fresh CI caches and wheel builds.
- **Docs:**
  - `dev-docs/platform-linux-wayland.md` §11.7: the smithay pin and its update policy, and EIS serving several clients through smithay's backend.
  - `CONTRIBUTING.md` §7, a short rule for git-pinned Rust dependencies.
- **Follow-up outside this change:** `fix-compositor-xwayland-startup-hang` should be revised after this lands, so that it kills a hung Xwayland through the exposed pidfd instead of documenting the gap. That change stays as it is here.
