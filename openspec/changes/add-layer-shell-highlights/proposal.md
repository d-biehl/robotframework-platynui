# Proposal

## Why

Highlighting an element under Wayland works only under the PlatynUI compositor. There it asks the compositor to draw the frame over its control socket (`show_highlight`). Every other compositor fails with "unsupported compositor", although most of them offer the standard way for a client to draw above all windows: the overlay layer of `zwlr_layer_shell_v1`. KWin, Sway, Hyprland, labwc, niri, cosmic-comp and the PlatynUI compositor itself all have it, as checked on 2026-11-26 against KWin 6.8, niri 26.04 and cosmic-comp 1.11. Only Mutter does not.

A highlight drawn as a layer-shell client works the same on every one of them. It is also testable in CI against the PlatynUI compositor, which already renders overlay-layer surfaces.

## What Changes

- **Highlights through `wlr-layer-shell`.** The Wayland highlight provider draws the frame as a client surface on the overlay layer, on every compositor that offers `zwlr_layer_shell_v1`, including the PlatynUI compositor. It looks as on X11, Windows and the compositor's own overlay: a red 3 px frame with a 1 px gap, dashed where it is clipped at the desktop edge. It spans every output the highlighted rectangles reach.
- **Never in the way.** The highlight surface takes no input, so clicks reach the windows below it. It never takes keyboard focus and claims no exclusive zone, so panels do not move it and it moves no window.
- **Duration and clearing as before.** A highlight disappears after its duration, a new highlight replaces the previous one, and clearing removes it at once. The highlight lives as long as the runtime that drew it; the CLI already keeps its process alive for the duration.
- **Gated by the protocol, not the compositor name.** A compositor without `zwlr_layer_shell_v1`, such as Mutter, fails a highlight with an unavailable error naming the layer shell and the compositor. Clearing still succeeds, as the existing gate requires.
- **The PlatynUI compositor's `show_highlight` stays** for `platynui-wayland-compositor-ctl` and other control-socket clients, but the platform no longer calls it.
- **One frame shape for X11 and Wayland.** The frame geometry (expand by gap and thickness, split into segments, dash the clipped edges) moves from the X11 backend into `platynui-core`, so both backends draw the same frame from one implementation.
- **Not BREAKING.** Highlights start working on more compositors. Under the PlatynUI compositor, the frame is drawn by a client surface instead of the compositor, with the same look.

## Capabilities

### New Capabilities

- `wayland-highlight-overlay`: how highlights are drawn under Wayland. It covers the overlay surface, how it looks, which outputs it covers, that it never takes input or moves windows, its duration and clearing, its lifetime, and when it is unavailable.

### Modified Capabilities

None. The `wayland-compositor-detection` requirement "A gated capability reports unavailable instead of success" keeps its wording. A compositor without a highlight backend is now one without `zwlr_layer_shell_v1`.

## Impact

- **Rust, `crates/platform-linux-wayland`:**
  - `highlight.rs`: the layer-shell overlay on its own background connection, replacing the control-socket call.
  - `Cargo.toml`: `wayland-protocols-wlr` is already a dependency with the `client` feature (the layer-shell module is in it).
- **Rust, `crates/core`:** the shared frame geometry in `platform/highlight.rs`.
- **Rust, `crates/platform-linux-x11`:** `highlight.rs` uses the shared geometry instead of its own (`expand_rect`, `frame_segments`).
- **`apps/wayland-compositor`:** no code change. The `show_highlight` command stays. Its integration tests gain no dependency on the platform crate.
- **Tests:**
  - unit tests for the geometry and the output mapping;
  - an integration test that highlights under a headless two-output PlatynUI compositor and checks the frame pixels in a screenshot;
  - a compositor-lane acceptance test that clicks a button through a highlight.

  The existing `BM.Highlight` calls in the egui smoke suite now go through the new path.
- **Docs:** `dev-docs/platform-linux-wayland.md` §7 and §15, and `dev-docs/architecture.md` §10.
- **Native rebuild:** yes (`just build-native`), no Python or Robot Framework API change.
- **Platforms:** Linux Wayland. X11 only gets the shared geometry, with identical output.
