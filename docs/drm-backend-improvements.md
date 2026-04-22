# DRM Backend Improvements

Tracking document for DRM backend improvements in the Wayland compositor.
Based on analysis of the current state and comparison with cosmic-comp.

## Phase 1 — Correctness & Stability

### 1. Session Pause/Resume (Critical)

VT switching (`Ctrl+Alt+F<n>`) currently only sets a boolean flag.
Without proper pause/resume, the DRM master may not be reclaimed and
input events leak through while switched away.

- [x] Store libinput context in `DrmBackendState` (currently consumed by calloop)
- [x] `SessionEvent::PauseSession`: call `libinput.suspend()`, `drm_device.pause()`
- [x] `SessionEvent::ActivateSession`: call `drm_device.activate()`, `libinput.resume()`
- [x] On resume: `schedule_render()` to repaint all outputs
- [x] Test: VT switch away and back, verify display resumes correctly

### 2. VBlank-timed Frame Callbacks (High)

Frame callbacks are sent in the render-ping handler (before the frame is
actually on screen).  Correct behavior: send after VBlank confirms
presentation.

- [x] Remove `send_frame_callbacks()` from render-ping handler
- [x] Add `send_frame_callbacks()` in `DrmEvent::VBlank` handler, after `frame_submitted()`
- [x] Fallback: outputs without a queued frame still need frame callbacks (timer or idle)
- [x] Verify clients (GTK4, egui) animate smoothly with VBlank-timed callbacks

### 3. Presentation Feedback with Hardware Timestamps (High)

`wp_presentation` is registered but unused in the DRM path.  The VBlank
handler receives `DrmEventMetadata` with real hardware timestamps but
ignores it (`_metadata`).

- [x] In `render_drm_outputs()`: call `state.take_presentation_feedback(&output, &result.states)` per output
- [x] Store collected `OutputPresentationFeedback` per output (between render and VBlank)
- [x] In VBlank handler: extract timestamp from `DrmEventMetadata`
- [x] Call `feedback.presented()` with hardware timestamp, `Kind::Vsync | Kind::HwClock`
- [x] Verify with `wayland-info` or `weston-presentation-shm` that timestamps are correct

### 4. Use Render Element States (Medium)

`DrmCompositor::render_frame()` returns `RenderElementStates` which are
currently ignored.  Three calls are missing compared to the winit backend.

- [x] Call `state.update_primary_scanout_output(&output, &result.states)` after render
- [x] Call `state.take_presentation_feedback(&output, &result.states)` (see item 3)
- [x] Call `state.send_dmabuf_feedback(&output, &result.states)` for DMA-BUF hints
- [x] Verify: no regressions with DMA-BUF clients (e.g. mpv, Firefox with dmabuf)

---

## Phase 2 — Robustness

### 5. Connector Hotplug (Medium)

Monitor plug/unplug is not detected.  `UdevEvent::Changed` only logs.

- [x] Handle `UdevEvent::Changed`: re-enumerate connectors on the DRM device
- [x] Detect newly connected connectors: create output + `DrmCompositor`
- [x] Detect disconnected connectors: tear down `DrmCompositor`, remove output from space
- [x] Relocate windows from removed outputs to remaining outputs
- [x] Notify output-management clients (wlr-randr, kanshi) about changes
- [x] Extract reusable parts from `initialize_drm_device()` for per-connector setup
- [x] Test: plug/unplug HDMI/DP monitor while compositor is running

### 6. GPU Hotplug (Low)

Only the primary GPU is initialized; additional GPUs are ignored.
Relevant for multi-GPU setups (e.g. eGPU, USB display adapters).

- [x] `UdevEvent::Added`: log; multi-GPU and late init not yet supported
- [x] `UdevEvent::Removed`: tear down device and all its outputs
- [ ] `UdevEvent::Added`: full init of additional GPUs (multi-GPU support)
  - Deferred: requires refactoring `State.drm_backend` from `Option` to
    `HashMap<u64, DrmBackendState>` (~15 call sites across 4 files) and
    storing `LoopHandle` in `State` for device init from udev callbacks.
- [ ] Handle renderer migration (surfaces from removed GPU need re-import)
  - Deferred: requires cross-GPU EGL context sharing / buffer re-import;
    no established pattern in the Smithay ecosystem yet.
- [ ] Test: eGPU attach/detach (if hardware available)

---

## Phase 3 — Performance

### 7. Per-Output Render Scheduling (Medium) — not planned

A single render-ping triggers rendering for ALL outputs at once.  With
mixed refresh rates (e.g. 60Hz + 144Hz), the faster output is starved.
Not relevant for UI testing scenarios (typically single output).

- [ ] Create per-output `Ping` (or store in `DrmOutputState` / `ActiveDrmCompositor`)
- [ ] VBlank per output triggers only that output's render
- [ ] Per-output frame callbacks instead of global `send_frame_callbacks()`
- [ ] Verify: two monitors at different refresh rates render independently
- [ ] Consider: per-output `send_frame_callbacks()` filtering by output

### 8. Hardware Cursor (Low) — already working

Smithay's `DrmCompositor` automatically assigns `Kind::Cursor` elements to
the hardware cursor plane.  Both xcursor theme icons and client cursor
surfaces already use `Kind::Cursor` (`render.rs` lines 336, 403).  The
DRM cursor size from GPU capabilities is passed to `DrmCompositor::new()`.
Smithay falls back to software compositing if the hardware plane is
unavailable.

- [x] Change xcursor `MemoryRenderBufferRenderElement` to `Kind::Cursor`
- [x] Verify Smithay places cursor on hardware plane (DRM debug: `LIBDRM_DEBUG=all`)
- [x] Test: cursor movement should NOT trigger full-frame re-render
- [x] Fallback: if hardware cursor fails, Smithay falls back to software automatically

### 9. Direct Scanout (Low)

Fullscreen DMA-BUF surfaces could bypass GL compositing entirely.

- [ ] Detect: single fullscreen window, DMA-BUF backed, no overlays/popups
- [ ] Try `test_only` atomic commit to verify hardware accepts the buffer directly
- [ ] If accepted: skip GL render, submit client buffer directly to primary plane
- [ ] Smithay's `DrmCompositor` may handle this via automatic plane assignment — investigate
- [ ] Test with fullscreen video player (mpv) — verify zero-copy scanout
- [ ] Fallback: if scanout fails, fall back to normal composited path

---

## Implementation Order

```
Phase 1:  1 → 2 → 3 → 4
Phase 2:  5 → 6
Phase 3:  7 → 8 → 9
```

Phase 1 items are interdependent (e.g. presentation feedback needs
render-element-states, VBlank frame callbacks feed into presentation
timing).  Consider committing as one or two logical changes.

Phase 2 and 3 items are independent and can be committed separately.
