# Design

## Context

See proposal.md for the motivation and `specs/wayland-highlight-overlay/spec.md` for the requirements.

**Today (verified in the code):**
- `WaylandHighlightProvider` (`crates/platform-linux-wayland/src/highlight.rs`) refuses every compositor but the PlatynUI one. It sends `show_highlight` or `clear_highlight` over the control socket; clearing succeeds everywhere.
- The PlatynUI compositor draws that frame itself (`apps/wayland-compositor/src/highlight.rs:6-17`): red, 3 px, 1 px gap, dashed (8 on, 4 off) where clipped at the output.
- The X11 backend draws the same frame with override-redirect windows on its own connection in a background thread (`crates/platform-linux-x11/src/highlight.rs`). It builds the frame with `expand_rect` (`:294`) and `frame_segments` (`:326`); Windows uses the same 3 px and 1 px (`crates/platform-windows/src/highlight.rs:251-252`).
- `HighlightRequest` (`crates/core/src/platform/highlight.rs:8`) carries rectangles in desktop coordinates and an optional duration.
- The CLI keeps its process alive for the duration (`crates/cli/src/commands/highlight.rs:68-72`). BareMetal keeps one runtime per library instance, so its overlay thread lives as long as the suite.
- The PlatynUI compositor offers `zwlr_layer_shell_v1` (`apps/wayland-compositor/src/state.rs:450`). It renders overlay-layer surfaces on top of everything (`render.rs:95`), so its screenshots include them. It can create several virtual outputs (`--outputs`, `src/lib.rs:146`).
- The platform crate already depends on `wayland-protocols-wlr` with the `client` feature, and reads output logical positions from `zxdg_output_v1` (`desktop/output_info.rs:24-27`).

## Goals / Non-Goals

**Goals:**
- One highlight implementation for every Wayland compositor with a layer shell, CI-tested against the PlatynUI compositor.
- The same frame on X11 and Wayland from one geometry implementation.

**Non-Goals:**
- Mutter: no client-side overlay exists there, and the highlight stays unavailable.
- Sharp frames on fractional or HiDPI scales (see Risks).
- Removing the compositor's `show_highlight` command.
- Placing elements correctly under foreign compositors. That depends on their window bounds (change `add-niri-cosmic-window-backends`). A highlight frames whatever desktop rectangles it is given.

## Decisions

### D1 — An overlay thread with its own Wayland connection

`WaylandHighlightProvider` owns a background thread with its own connection, like the X11 provider. The first highlight starts it, and dropping the provider stops it.

On that connection the thread binds:
- `wl_compositor`, `wl_shm` and `zwlr_layer_shell_v1`;
- the outputs with `zxdg_output_v1`.

`highlight()` and `clear()` send it a command and wait for its acknowledgement. A failure (no layer shell, a lost connection) therefore reaches the caller as an error, and a surface error does not vanish in the thread. Availability is decided from the bound globals, not from the compositor's identity.
- *Rejected:* drawing on the caller's thread. The surfaces must be kept and redrawn after the call returns, for the duration, and replaced by later calls.

### D2 — One full-output surface per output the highlight reaches

For every output that an expanded rectangle intersects, the thread creates a layer surface with:
- the overlay layer and the namespace `platynui-highlight`;
- anchors on all four edges, so the surface covers the output and its coordinates are the output's logical coordinates;
- exclusive zone `-1`, so other surfaces' zones do not shift it;
- no keyboard interactivity, and an empty input region.

The buffer is a transparent `ARGB8888` `wl_shm` buffer in the output's logical size, with the frame segments of all rectangles drawn in red. Segments are translated from desktop to output coordinates by subtracting the output's logical position. A segment that crosses an output edge is cut there and continues on the neighbour, so a rectangle across two outputs is framed without a gap.
- *Rejected:* one small surface per rectangle, positioned with margins. A frame across outputs would need several surfaces anyway, and margins are measured from the anchored edges, which makes the position depend on how each compositor applies them.
- *Rejected:* `tiny-skia` for drawing. Axis-aligned filled rectangles are a few loops over the buffer.

### D3 — The frame geometry moves into `platynui-core`

`expand_rect` and `frame_segments` move from `crates/platform-linux-x11/src/highlight.rs` to `crates/core/src/platform/highlight.rs`. They stay pure functions over `Rect`, with the existing constants (3 px, 1 px, dash 8/4). The X11 provider calls them from there with unchanged output; its tests move with them. The Wayland overlay uses the same functions with the desktop bounds as the clip area.
- *Rejected:* a third copy. The compositor keeps its own, because it renders with smithay elements and does not depend on `platynui-core`.

### D4 — Duration, replacement and lifetime

- **Replacement.** A new highlight replaces the surfaces' content: surfaces for outputs no longer reached are destroyed, and the remaining ones are redrawn.
- **Duration.** The thread keeps a deadline for it and destroys all surfaces when it passes, unless a newer highlight replaced it.
- **Clearing.** `clear()` destroys all surfaces at once, and always succeeds: when the overlay thread never started or no layer shell exists, there is nothing to clear.
- **Lifetime.** When the runtime ends, the connection closes and the compositor removes the surfaces, so a highlight never outlives its runtime.

### D5 — The platform stops calling `show_highlight`

`WaylandHighlightProvider` no longer reads the compositor type and no longer calls the control socket. The compositor's `show_highlight` and `clear_highlight` stay for `platynui-wayland-compositor-ctl` and other clients. The identification-based gate for highlights in `capabilities.rs` goes away. The protocol gate replaces it with the same error shape: unavailable, naming the capability and the compositor.

### D6 — How it is tested

- **Unit tests:**
  - the moved geometry with the X11 cases;
  - the desktop-to-output mapping, including the cut at a shared edge;
  - the availability decision over a list of globals.
- **Integration test in the platform crate:**
  - starts a headless PlatynUI compositor with two outputs through the existing harness pattern (`tests/pidns_tests.rs`, `scripts/wayland-sidecar-harness.sh`);
  - highlights through the CLI and captures through the CLI's screenshot command;
  - checks frame pixels for the spec's rectangles, their absence after the duration and after clearing, and the replacement.
- **Compositor-lane acceptance tests (egui):** a click through a highlighted button, and typing into a focused field while its window is highlighted.

## Risks / Trade-offs

- **[A buffer per output costs memory while a highlight is shown]** (8 MB at 1920×1080, 33 MB at 3840×2160) → It exists only while a highlight is shown and is released on clear or when the duration passes.
- **[On scaled outputs the frame is drawn at logical size and scaled by the compositor, so it may look soft]** → It is a highlight, not a measurement. `wp_viewporter` or `wp_fractional_scale_v1` can sharpen it later without changing behavior.
- **[A compositor may restrict the overlay layer or reorder it]** → A refused surface surfaces as an error through D1's acknowledgement. KWin, niri and cosmic-comp accept overlay surfaces from ordinary clients (their panels and launchers use them).
- **[Screenshots taken while a highlight is shown include the frame]** → This was already true with the compositor-drawn frame. Tests that compare screenshots clear first, as before.
- **[The X11 refactor could change X11 frames]** → The moved functions keep their tests and constants, and the X11 lanes run unchanged.

## Migration Plan

- **Kind of change:** behavioral for the PlatynUI compositor (a client surface draws the frame instead of the compositor, with the same look), additive for other compositors (highlights start working).
- **Native rebuild:** yes (`just build-native`). No Python or Robot Framework API change.
- **Rollback:** revert the change. The compositor still has `show_highlight`, so restoring the old provider needs no compositor change.
