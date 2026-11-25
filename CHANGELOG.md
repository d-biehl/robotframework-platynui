# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

## [Unreleased]

### Bug Fixes

- **BareMetal:** Convert duration from seconds to milliseconds for highlight method ([9f3d1c7](https://github.com/imbus/robotframework-PlatynUI/commit/9f3d1c7f350ce9dcd100dd42becf1767bde52a95))
- **assertable:** Insert assertion params before keyword-only params ([52e35eb](https://github.com/imbus/robotframework-PlatynUI/commit/52e35eb3950d5cf9f89697743930bf766fe13f4b))

  The decorator appended its positional-or-keyword assertion parameters
  (assertion_operator/expected/message) to the end of the wrapped
  signature. That breaks for any keyword that has keyword-only parameters
  (after a bare `*`): a positional-or-keyword parameter cannot follow a
  keyword-only one, so building the signature raised ValueError.

  Insert the assertion parameters before any trailing keyword-only,
  *args and **kwargs parameters instead. Behaviour is unchanged for the
  existing assertable keywords (none had keyword-only parameters); this
  unblocks adding one.

- **atspi:** Prevent CLI hang by replacing async-io timeout with OS-level park_timeout ([b7c4579](https://github.com/imbus/robotframework-PlatynUI/commit/b7c45794f4a8c1050373ad76c57d40c9b59fae4c))

  The `block_on_timeout` helper used `futures_lite::block_on` racing the
  D-Bus future against an `async_io::Timer`. When combined with zbus v5's
  internal executor (which shares the same async-io reactor), the timer
  waker could fail to fire, causing `platynui-cli query` to hang
  indefinitely after exhausting the AT-SPI tree.

  Replace the implementation with a manual poll loop using
  `std::thread::park_timeout`, which enforces the deadline via OS-level
  thread parking independent of any async runtime.

- **atspi:** Standardize namespace handling in AttrsIter ([a7b3827](https://github.com/imbus/robotframework-PlatynUI/commit/a7b382709a085a2596f4d6cb3b19ed5ad73553ab))
- **baremetal:** Do not start assertion failures with None ([72dbf71](https://github.com/imbus/robotframework-PlatynUI/commit/72dbf71ba1da74d8b46010a0609b5524308cd268))

  An assertable keyword passed a missing assertion_message on as the
  message prefix, so its failure started with the text None.

- **baremetal:** Contain Robot Framework's untyped surface in _registered_name ([d451279](https://github.com/imbus/robotframework-PlatynUI/commit/d4512797157d34cdb4df865a3f52de57687162b1))

  Robot Framework ships no py.typed, so pyright infers Unknown for
  everything reached through the execution context and reported the
  propagation into getattr() (reportUnknownArgumentType). A plain
  annotation does not help — pyright narrows on assignment to the
  assigned type — so cast the namespace to `object` once and narrow
  explicitly from there.

  Drops a dead `# pyright: ignore[reportUnknownVariableType]`: that rule
  is switched off project-wide, so the comment suppressed nothing.

  No behaviour change; deliberately no Protocol or stub for the private
  `_kw_store` / `_instance` attributes, which would dress an unstable
  private surface up as a typed contract.

- **baremetal:** Don't cache a failed library-name lookup ([b24d2b6](https://github.com/imbus/robotframework-PlatynUI/commit/b24d2b6f27dc3c74c6b74c69d466812f0fc14006))

  _variable_suffix() remembered whatever the first lookup produced,
  including a miss. When the registered name cannot be resolved — no
  execution context, or the instance not yet in the namespace — the
  unsuffixed fallback was cached for the lifetime of the instance, so an
  aliased import would read and write the default import's variable from
  then on. That is the leak the per-import naming exists to prevent, and
  it would be silent, since the import stays internally consistent.

  No path inside a running keyword is known to reach it (the namespace
  lookup even covers an Import Library ... AS <alias> mid-test), but the
  failure mode is bad enough to close: only a resolved name is cached,
  a miss falls back for that call alone.

- **baremetal:** Isolate scoped state per library import ([0d4c6dd](https://github.com/imbus/robotframework-PlatynUI/commit/0d4c6dd1592fff7768e14747742b4761a05f21a3))

  Set Root stored its root in a single globally named Robot Framework
  variable, so every BareMetal instance read the same cell: one import's
  root silently became every other import's root. Set Query Settings
  leaked the same way. The repo's own acceptance suites already hit this
  (coexisting_runtimes.robot, dedup.robot), where a BM.Set Root at suite
  scope also governed the second runtime.

  Isolating the variable is not enough, because a UiNodeDescriptor welded
  together three things with different lifetimes: a selector (pure data),
  a resolved element (bound to one Runtime) and its evaluation context
  (library, overrides). The context was captured at creation and used at
  resolution, so a descriptor read from a variable resolved through the
  runtime and root of whoever created it. Within a single import, the
  resolved element was cached on the shared per-query descriptor keyed by
  the selector string alone, so the same relative selector under a
  changed root returned the element from the previous root.

  - Scoped variables are named after the registered library name: the
    default import keeps ${PLATYNUI_ROOT_DESCRIPTOR}, an alias gets a
    normalized _<ALIAS> suffix. Stable across the per-suite instances a
    suite-scoped library goes through.
  - scope=SUITE now propagates to child suites (children=True), so a
    directory's __init__.robot can pin the context for the suites below.
    Stored values carry an import fingerprint; a value inherited by a
    differently configured import is ignored with a warning.
  - library and overrides move off the descriptor into resolve(), so a
    reference is always resolved by the library whose keyword is running.
  - Implicit element caching is gone; a capture is explicit and pinned.
    Runtime.instance_id and UiNode.owner_id make an element from another
    runtime detectable, which pure Python could not do.
  - The root is only resolved when the selector actually needs it: an
    absolute selector ignores the context node, and resolving a vanished
    root for it made lookups fail that never needed it.

- **baremetal:** Fall back to bounds center when ActivationPoint is absent ([8aee2ad](https://github.com/imbus/robotframework-PlatynUI/commit/8aee2ad1f04cddad80eb660ffb99a9432e5dba3f))

  UiNode.attribute raises AttributeNotFoundError for missing attributes, so
  the bounds-center fallback in _resolve_screen_point was dead code: pointer
  keywords targeting a node without an ActivationPoint (e.g. `Pointer Click .`
  on the Desktop root) failed with AttributeNotFoundError instead of clicking
  the element center.

  Catch AttributeNotFoundError per lookup and fall back to Bounds. Use
  Rect.center() (float center) instead of the previous integer floor center
  so the fallback matches the provider-computed ActivationPoint.

- **baremetal:** Scope UiNodeDescriptor cache per library ([cbde461](https://github.com/imbus/robotframework-PlatynUI/commit/cbde46178226a164838ee630813ab026f2115513))

  Move UiNodeDescriptor caching from a global class-level map into the
  owning BareMetal instance.

  This keeps descriptor resolution isolated per runtime/library context
  and prevents query strings from leaking cached nodes across libraries.

- **build:** Use --no-sync for uv run in wheel build recipes ([c8a1d24](https://github.com/imbus/robotframework-PlatynUI/commit/c8a1d24977c91ac9b973382336a0a3194f157dd1))

  Prevent uv from re-syncing the virtual environment when building
  wheels. On CI runners where Python is provided externally (e.g.
  Windows ARM via setup-python), uv run may detect a mismatch and
  recreate the venv, losing maturin which is only declared as a
  dev-dependency in sub-packages. Using --no-sync after the initial
  bootstrap avoids this issue.

- **build:** Add pkg-config dependency and build script for xkbcommon ([22adba3](https://github.com/imbus/robotframework-PlatynUI/commit/22adba3ae93e83736bf5655b88648d29f4b7f1e5))
- **ci:** Install libxkbcommon-x11 for the X11 egui acceptance lane ([b44e897](https://github.com/imbus/robotframework-PlatynUI/commit/b44e897aff68635fd042171c8c171131aed92354))

  winit's X11 backend dlopens libxkbcommon-x11.so.0 (via xkbcommon-dl). The runner
  had libxkbcommon-dev (the base libxkbcommon.so) but not the separate X11 helper
  package, so the egui test app panicked at startup with 'Library libxkbcommon-x11.so
  could not be loaded' before creating its window. AccessKit therefore never
  registered, and all 17 X11 acceptance tests failed with 'not on the accessibility
  tree'. The Wayland lane is unaffected (its winit backend only needs libxkbcommon.so).
  Root cause confirmed via the egui app log dumped by the new failure diagnostic.

- **ci:** Add libinput and libgbm dependencies for Linux builds ([8d77718](https://github.com/imbus/robotframework-PlatynUI/commit/8d77718dcf3d6c362ec437450b4a207bfeed8c26))
- **cli:** Correct help text for keyboard sequence arguments ([43679ed](https://github.com/imbus/robotframework-PlatynUI/commit/43679edcb8d95b336c7b7dc8455c4929d7cd8aa3))
- **compositor:** Survive clients from foreign PID namespaces ([d8db766](https://github.com/imbus/robotframework-PlatynUI/commit/d8db7662bcdf13443ece0c559efca9e4beb18689))

  The compositor aborted the whole session as soon as it had to report a
  client whose process it could not resolve. `window_pid` asked
  `wayland_server::Client::get_credentials`, which reads SO_PEERCRED through
  a rustix whose `UCred.pid` cannot represent the `0` the kernel reports for
  a peer in a PID namespace the compositor cannot see: the read panicked
  inside wayland-backend, the poisoned backend mutex was unwrapped on the
  next poll, and the panic in the destructor turned into a process abort.
  Measured in a sidecar topology, a single `list_windows` killed the session
  and every client in it.

  The compositor now establishes a client's identity itself, once, at the one
  moment it still owns the accepted socket — the Wayland listening socket and
  the `wp-security-context-v1` listener — and keeps it with the per-client
  data. `SO_PEERCRED` is read through `nix`, which can represent `0`, so no
  `unsafe` enters the crate. An unresolvable process is reported as JSON
  `null`; `0` is never emitted, because a consumer filtering by process would
  match against it, and a process id an XWayland client declares as `0` is
  normalized the same way. The accessor that panicked is banned through
  clippy's `disallowed-methods` so it cannot come back.

  `window_at_point` no longer answers with a window of the process that
  asked. It walks the stack front to back with the same hit criterion smithay
  applies and returns the frontmost window the caller does not own, or
  nothing. Both sides of that comparison are identities the compositor
  established itself — the client's when it accepted the connection, the
  caller's from the control connection the request arrived on — and a window
  is skipped only on a positive match: an unidentified client, an
  unidentified caller, or a process id an XWayland client merely declared
  about itself exclude nothing. The listings keep reporting the caller's own
  windows; only the point lookup excludes.

- **compositor:** Exit with the child's result under --exit-with-child ([ae5f614](https://github.com/imbus/robotframework-PlatynUI/commit/ae5f614b0a68d7034a824851e30fb34912f17b35))

  With --exit-with-child the compositor always exited 0: the child monitor
  only logged the child's exit status. The Wayland acceptance lane runs
  Robot Framework as that child, so a failing run came out as success and
  the CI compositor job could never go red; the X11 lane passed the code
  through all along.

  The compositor now reports the child's result as its own exit code,
  following the shell conventions:

  - the child's exit code, or 128 + n if a signal n terminated it;
  - 127 (not found), 126 (not executable) or 1 if the child cannot be
    started, and the session ends right away instead of idling with
    nothing to wait for;
  - 1 if the session ends while the child is still running (--timeout, the
    IPC shutdown command, SIGTERM/SIGINT, closing the window), because an
    unknown result must never read as success.

  Without --exit-with-child the exit code stays 0. The monitored child now
  lives in State, and the exit code is taken after the event loop stops
  with one last check of the child, so an exit that lands between two
  100 ms polls is not lost.

  As a consequence `just test-acceptance` on Linux now stops after a
  failing compositor lane, like any just dependency chain; CONTRIBUTING and
  the recipe comment say so, and the robot-test-style skill no longer calls
  lane exit codes unreliable. The exit code contract is documented in the
  compositor's usage docs and covered by tests that run the compositor
  with a real child.

- **compositor:** Keep sub-pixel deltas in interactive move grabs ([c1aa268](https://github.com/imbus/robotframework-PlatynUI/commit/c1aa26823b638c60d77f73079c7a50e95a2831a5))
- **compositor:** Fall back to surfaceless EGL for offscreen screenshots ([e9d0b9e](https://github.com/imbus/robotframework-PlatynUI/commit/e9d0b9ec0d14f300ae316104c7a64269c1e3568a))

  The offscreen screenshot renderer built its EGL display via GBM, which is a
  userspace wrapper around a kernel DRM driver and therefore requires a /dev/dri/*
  render node. On a headless GPU-less runner no such node exists, so screenshot
  capture failed with 'no DRI render node found' — LIBGL_ALWAYS_SOFTWARE=1 cannot
  help, since it only affects rendering, not GBM's device requirement.

  Prefer GBM when a render node exists (GPU-accelerated, also fine with llvmpipe),
  and otherwise fall back to Mesa's surfaceless EGL platform
  (EGL_PLATFORM_SURFACELESS_MESA), which renders into client memory via llvmpipe and
  needs no device node. The offscreen renderer composites into an FBO and never
  creates a window surface, so a configless surfaceless context is sufficient.

- **compositor:** Reconfigure maximized windows on winit resize ([f2f377c](https://github.com/imbus/robotframework-PlatynUI/commit/f2f377ce224b7a3c06d09f292ea2bcebfa8e8a24))

  The single-output winit resize branch updated the output mode but did
  not call reconfigure_windows_for_outputs(), so maximized and fullscreen
  windows kept their original dimensions after resizing the compositor
  window. Move the reconfigure call outside the if/else so it runs for
  both single-output and multi-output resize events.

  Additionally, reconfigure_windows_for_outputs() only processed Wayland
  toplevels and silently skipped X11 (XWayland) windows. Extend the
  function to also handle maximized and fullscreen X11 surfaces via
  x11.configure(), ensuring they adapt to the new output geometry.

- **compositor:** Restore X11 window size on unmaximize ([12d7ff5](https://github.com/imbus/robotframework-PlatynUI/commit/12d7ff5e2c97d49c52dad8611abb0c0ee7f28e45))

  Maximizing then un-maximizing an X11 window restored the position but
  left the window at full-screen size.

  Root causes:
  - `pre_maximize_positions` only stored position (no size), so X11
    unmaximize called `x11.configure(None)` which does not restore
    dimensions (unlike Wayland clients that negotiate size via protocol).
  - XwmHandler `maximize_request` / `unmaximize_request` were not
    implemented, so X11 apps requesting maximize via `_NET_WM_STATE`
    were silently ignored.
  - `remove_x11_window` did not clean up `pre_maximize_positions`,
    leaving stale entries for destroyed windows.

- **compositor:** Fix VNC cursor rendering for surface and SSD cursors ([b5ac98d](https://github.com/imbus/robotframework-PlatynUI/commit/b5ac98da9f8c9b0e392aaf17d285e64ae71c4ce8))

  The screencopy cursor session sent transparent pixels for two cases
  that the main render pipeline handled correctly:

  1. Surface cursors (XWayland/X11 clients using wl_pointer.set_cursor):
     render the cursor surface through the offscreen GL pipeline and copy
     the pixels into the capture buffer instead of filling with transparent.

  2. Compositor-driven SSD cursors (resize borders, move grab): check
     compositor_cursor_shape before cursor_status, matching the priority
     chain used by the render pipeline. Without this, resize cursors at
     SSD window edges were invisible in VNC.

  Also export compositor_cursor_icon() from the render module so the
  screencopy handler can reuse the same CursorShape-to-CursorIcon mapping.

  Additionally set XKB_DEFAULT_LAYOUT in the VNC session script so wayvnc
  uses the correct keyboard layout for keysym-to-keycode translation.

- **compositor:** Map virtual pointer coordinates to bound output ([6eab4f9](https://github.com/imbus/robotframework-PlatynUI/commit/6eab4f93d3887a656bdc72ddbff0d2701ca50a50))

  CreateVirtualPointerWithOutput previously ignored the output parameter,
  mapping motion_absolute coordinates to the full combined geometry of all
  outputs.  This caused VNC clients like wayvnc to produce wrong cursor
  positions in multi-monitor setups — the pointer was stretched across the
  entire desktop instead of being confined to the captured output.

  Store the bound output in VirtualPointerUserData when a virtual pointer
  is created with CreateVirtualPointerWithOutput, and use that output's
  geometry in handle_motion_absolute.  Fall back to combined geometry for
  unbound pointers or when the bound output is no longer mapped.

- **compositor:** Correct popup constraining for SSD and layer-shell popups ([0721239](https://github.com/imbus/robotframework-PlatynUI/commit/0721239be1b3afa25f6a1aee4f8715f3194665a2))

  Fix three popup issues found with Kate (SSD) and ironbar (layer-shell):

  - pointer_hit_test() now uses element_bbox() to include popup geometry,
    so pointer events reach popups extending beyond SSD window frames
  - unconstrain_popup() computes constraint rects in popup-parent-relative
    coordinates for both window and layer-surface branches
  - Strip ResizeX/ResizeY from layer-surface popup constraints to prevent
    GTK4 GtkPopover from destroying itself when content exceeds screen
    height (e.g. ironbar submenus with many items)

  Add popup_destroyed handler and improve popup debug logging.

- **core:** Show a whitespace-only id in one-line descriptions ([ebeecff](https://github.com/imbus/robotframework-PlatynUI/commit/ebeecfff99e3198671cd41711910985058809a67))

  An identifier made of whitespace is a value like any other, so the
  one-line description of an element, as action log lines use it, now
  shows it after the '#'. Only an empty id is left out.

- **core:** Discard the snapshot when PlatynUI.core finds nothing ([0c2d805](https://github.com/imbus/robotframework-PlatynUI/commit/0c2d805cdc555624f9c1860d1c41b73d2619e988))

  The runtime answers from a snapshot that the next query may reuse, and
  only the caller knows when it no longer fits. The object model never
  discarded it, so a retry looked at the same stale snapshot until its
  timeout. A lookup that finds nothing now discards it, and a context
  discards it before each retry and when it gives up; a lookup that
  succeeds keeps it, so the reuse still pays off.

- **core:** Make serde_json a dev-dependency ([a8f95fb](https://github.com/imbus/robotframework-PlatynUI/commit/a8f95fb0f85f9f10f7f616561107776bb7e61ad2))

  Only the serde round-trip tests use it, so as a normal dependency it
  was unused in the library and shipped to every downstream crate for
  nothing. The lint missed it because core does not inherit the
  workspace lints.

- **core:** Honour explicit raise_exception on nested ensure_that ([b287999](https://github.com/imbus/robotframework-PlatynUI/commit/b28799911d60a2e8afc68d61894b4abeb46dbe09))

  A nested ensure_that() call now keeps its own raise_exception when one
  was supplied explicitly; only the default (None) still inherits from
  the outermost scope. This makes exists(raise_exception=False) safe to
  use inside stricter outer verification blocks (e.g. Window.close ->
  _window_is_gone -> self.exists()), where a missing window must surface
  as False instead of propagating CannotEnsureError.

  Also extend the ensure_that test suite with seven nested-scope cases
  covering raise inheritance, timeout sharing, three-level chains, the
  shared succeeded memo, start_time ownership and depth restoration.

- **core:** Eliminate UB in downcast_pattern_arc ([673453c](https://github.com/imbus/robotframework-PlatynUI/commit/673453c9716f63b5eb8d7a83f3d79fd88bbe3f87))

  Replace unsound fat-pointer cast from `*const dyn UiPattern` to
  `*const (dyn Any + Send + Sync)` which reinterprets the vtable — this
  is undefined behavior since the two trait objects have different vtable
  layouts.

  Cast directly to `*const T` (thin pointer) after the `is::<T>()` type
  check, matching the pattern used by `std::sync::Arc::downcast`.

- **dependencies:** Restrict robotframework-robocop version to <8.0.0 ([ffdbc7b](https://github.com/imbus/robotframework-PlatynUI/commit/ffdbc7b6039653e1ee235e1c4c6ee3947532e8b5))
- **evaluator:** Enhance boolean evaluation for node sequences ([3d8b85a](https://github.com/imbus/robotframework-PlatynUI/commit/3d8b85a81a9df54c7ab0fd5f43acb83fda8d6d61))
- **hooks:** Pin PyO3 to uv Python ([a251080](https://github.com/imbus/robotframework-PlatynUI/commit/a251080609971defb0f6b08087146fe65229bd07))
- **hooks:** Skip Linux cross-target gate on Windows ([69a3a8a](https://github.com/imbus/robotframework-PlatynUI/commit/69a3a8afbb76f4ed1c5a213ac14343d9ea837def))
- **inspector:** Make the fallback modifier reader uninhabited ([d0724b0](https://github.com/imbus/robotframework-PlatynUI/commit/d0724b00de22faafffa0d46136e40c5d242be0ee))

  Where the Inspector has no global modifier reader (macOS today),
  ModifierReader::new never builds one, yet the type was an empty struct
  whose read method ignored self, which clippy rejects when linting for
  macOS. An enum without variants states what is true: no value exists, so
  read matches on it instead of ignoring it. Linux and Windows are
  unchanged.

- **inspector:** Make the live mouse picker work on Windows ([f26ffa8](https://github.com/imbus/robotframework-PlatynUI/commit/f26ffa81b56b5be8f5d2726993da17f0db56f876))

  The element-at-point picker resolved elements but could not select them
  in the tree on Windows, and surfaced noise. Fixes:

  - Reveal-and-select needs the picked node's parent chain, which an
    out-of-tree ElementFromPoint result lacks: the UIA provider now wires
    an app-scoped ancestor chain (UiaNode::attach_ancestor_chain) so the
    existing reveal path can walk it and match tree runtime ids.
  - Own-process UI resolves to nothing (keep the Inspector off the
    target); no tunnel-behind. The highlight overlay is WS_EX_TRANSPARENT
    so ElementFromPoint passes through it.
  - Immersive Win11 shell windows (Start menu, taskbar / jump-list /
    settings flyouts) are enumerated via FindWindowExW, since EnumWindows
    omits them; gated on being on-screen (not WS_EX_NOACTIVATE).
  - The picker reveal no longer clears every level each tick (only reloads
    a level when the wanted node is missing), so the top-level window/app
    list stops churning while a modal menu blocks UIA; and it no longer
    accumulates expanded paths (arm-time snapshot, reset per pick).
  - Dynamic menus (Windows shell / VS Code) hand out unstable UIA
    RuntimeIds, so reveal-by-id diverges; the resolved live chain is then
    grafted into the tree (UiNodeData::graft_chain) rather than giving up.
  - Zero-area phantom windows are filtered out (window_has_area).

  Acceptance lane: the Windows recipe and the Linux session script now
  build the Inspector and hand over PLATYNUI_INSPECTOR_BIN; the default
  test-app / inspector bin vars resolve an absolute, OS-native path
  (CreateProcess will not launch a relative, .exe-less path on Windows).
  Adds unit tests for resolved_chain_below and graft_chain and asserts
  selection in inspector_picker.robot. OpenSpec proposal/design/tasks
  updated to match the implemented approach.

  Known limitation: Chromium menu-bar dropdowns (e.g. VS Code) return the
  widget beneath the menu overlay via ElementFromPoint, so those items are
  not pickable; shell and context menus work.

- **inspector:** Drop control namespace prefix in attribute display and copy ([7f5aee7](https://github.com/imbus/robotframework-PlatynUI/commit/7f5aee70de9fd436d546267363971afedaa75525))

  Control is the default attribute namespace in PlatynUI XPath, but the engine
  follows standard XPath rules: unprefixed @Foo matches control attributes,
  @control:Foo does not. The inspector previously rendered and copied the
  prefixed form, so pasting a copied attribute name into a query returned no
  results. Strip the prefix in user-facing strings (ungrouped column, search
  result labels, Copy Name, Copy Row) while keeping the prefixed form for
  internal pin/filter identity.

- **inspector:** Avoid Windows startup stalls ([2296412](https://github.com/imbus/robotframework-PlatynUI/commit/22964126c302fdb2c798847f93f462594c9111eb))

  Defer initial tree traversal behind the first rendered frames and load tree children incrementally so provider work does not block startup.

  Configure the Windows wgpu default to use Vulkan or DX12 while keeping OpenGL available through WGPU_BACKEND=gl for diagnostics.

- **inspector:** Keep arrow keys in the XPath search field ([3b0fc45](https://github.com/imbus/robotframework-PlatynUI/commit/3b0fc4539028808113e1f80dfa7a91525f8f02bb))

  Capture horizontal and vertical arrow keys in the search field focus
  lock filter so focus stays in the multiline XPath editor instead of
  jumping to other controls.

- **inspector:** Update some dependencies and fix scrollbar clipping in properties and results panels ([67eadc9](https://github.com/imbus/robotframework-PlatynUI/commit/67eadc9a05e01bf8d5ecb8f19e0a3e36df734687))

  The egui_extras TableBuilder renders its header row outside the body
  ScrollArea. Setting max_scroll_height to available_height caused the
  total height (header + body) to exceed the panel, clipping the
  scrollbar and making the last row unreachable.

- **inspector:** Send window level command only on state change ([0038e98](https://github.com/imbus/robotframework-PlatynUI/commit/0038e98d73f33af1e2062db043c4dda8185d7d6a))

  The "Always On Top" viewport command was sent every frame, causing
  ~120 _NET_WM_STATE ClientMessages per second on X11/XWayland. This
  flooded the compositor log at DEBUG level and wasted resources.

  - Only send the WindowLevel viewport command when the always-on-top
    setting actually changes, not on every frame
  - Move the check after toolbar rendering so checkbox toggles take
    effect in the same frame

- **java-agent:** Send empty strings instead of dropping them ([0a788b7](https://github.com/imbus/robotframework-PlatynUI/commit/0a788b7b3b7185b44d54eff513e7fe18e4fd3f81))

  The agent dropped every empty string from the wire, so the native
  attributes could not show an empty accessible name, tool tip,
  component name or window title that Swing reported. It now leaves out
  only null. Whether an empty Id or Description counts as none stays the
  provider's decision, and new provider tests pin that it does.

- **java-agent:** Stop calling an unreadable module list no JVM ([999ce95](https://github.com/imbus/robotframework-PlatynUI/commit/999ce95f42fd71f3e67a57cbbbd83cd8e879adff))

  A JVM loads libraries all through its start-up, and Windows fails a
  module snapshot taken while that happens, asking the caller to retry.
  The attach turned that failure into NotAJvm: an automatic attachment
  then lost one of its attempts, the pass went to the Access Bridge, and
  the live tests failed now and then. The snapshot is now taken again for
  a short while; a list that still cannot be read fails the attach with
  its Windows error instead, and only a list without jvm.dll means "not a
  JVM". The Java classifier asks the same probe, and a process without a
  JVM is no longer opened with the rights to start a thread in it.

- **java-agent:** Leak the attach stub while its thread may still run ([7fcfb8d](https://github.com/imbus/robotframework-PlatynUI/commit/7fcfb8de81c197e75bd664b0f7091d6854fa77e4))

  When the remote thread that loads the agent has not finished in time,
  freeing its stub could crash the target JVM. The stub is now leaked in
  that case, and the timeout, which is returned, is logged at debug.

- **java-agent:** Repair the Web Start harness's JDK assumptions ([395e5ef](https://github.com/imbus/robotframework-PlatynUI/commit/395e5ef636bc70b7ed920bd1c10415e3adc04300))

  The harness chose its JDK by sorting directory names, which ranks 8 above
  21, and its JDK-9+ attach path could never have compiled. Neither showed
  here because JAVA_HOME already pointed at a JDK that fit — so the harness
  we rely on to keep the automated Web Start model honest was itself only
  accidentally working.

  Also gives `java21` in java-launchers.properties the consumer it was
  documented as having.

- **java-agent:** Report Swing client properties again ([7a89186](https://github.com/imbus/robotframework-PlatynUI/commit/7a89186ae43585ea7d5f03b211907d9cb684b654))

  The reflective lookup named a method signature that does not exist, so the
  block has been empty on every JDK since it was written. Nothing noticed,
  because an optional block that stays empty looks exactly like an
  application that sets no client properties — while for enterprise Swing
  these are often a control's only stable identifier.

- **java-agent:** Run independently of the target's security policy ([ff28513](https://github.com/imbus/robotframework-PlatynUI/commit/ff2851314abaf6b20eb1aa02ba52dfab89ee3dfc))

  A Java Web Start target installs a JNLPSecurityManager, and the agent JAR
  is appended to the SYSTEM class path by the attach — so the agent's own
  code runs under the application's sandbox. Measured against OpenWebStart
  1.14 on Java 8, it died on its first statement:

      Denying permission: ("java.lang.RuntimePermission"
                           "getenv.PLATYNUI_AGENT_DIR")

  while the attach reported success, so the provider saw a JVM that simply
  had no agent. Worse, the report of that failure threw its own failure:
  AgentLog's static initializer reads a system property, which the same
  policy denies, so the JVM raised an ExceptionInInitializerError — an
  Error, outside the old catch — and it escaped agentmain into the target's
  attach listener thread.

  Boot-Class-Path puts the agent's classes on the bootstrap loader, which
  defines them with a null protection domain and therefore all permissions.
  This widens nothing: loading an agent into a JVM is already unrestricted
  code execution in that process, which is what JEP 451 exists to restrict,
  and it is what the JNLP runtime does for its own code. Verified against
  the real OWS application: with the attribute the agent starts, publishes
  and serves; without it, it does not.

  Three consequences, each a correctness fix in its own right:

  - The version resource can no longer be read through getClassLoader(),
    which is null for a bootstrap-defined class. Going through the class
    resolves via the system class loader, so the JAR being on BOTH search
    paths is load-bearing rather than incidental — recorded at the call
    site, because a test that simulates this with -Xbootclasspath/a: alone
    would read "unknown" and look like it found a bug.
  - The diagnostic channel must survive a denied property read. It is the
    last thing that still works when everything else has failed.
  - No Throwable may reach the application. AgentRuntime.start now guards
    the CONSTRUCTION too — that is what first touches AgentPaths' and
    RpcServer's class initializers, and an initializer that throws is
    exactly the shape that escaped before. Agent.start carries a last-resort
    catch on top, because a failure raised while reporting an earlier one
    cannot be caught by the single catch whose own body does the reporting.

  Agent.started is deleted rather than repaired: AgentRuntime already keys
  idempotence on a live instance, which is the correct key. The flag keyed
  on "an attempt was made" and was never cleared, so one transient failure
  made every later injection a silent no-op and the client's retry budget
  was spent on a JVM that had quietly decided never to answer.

  Windows acceptance lane 115/115 with the rebuilt JAR staged, agent JUnit
  suite green. The AppContext half of Web Start support is not in here.

- **java-agent:** Stop agent discovery from flashing a console window ([5b18a9f](https://github.com/imbus/robotframework-PlatynUI/commit/5b18a9f901f9f62998314565ea1921e98a225be8))

  Discovery asks the environment's interpreter once per process which
  providers it has registered. `python.exe` is a console-subsystem
  program, so Windows hands such a child a *fresh console* when the parent
  has none — which is exactly the Inspector's situation. The output was
  always captured over a pipe, so the window was pure noise: a black box
  flashing over the very application the user is inspecting.

  `CREATE_NO_WINDOW` suppresses it without changing anything about the
  query. Not solved by preferring `pythonw.exe`: that name only exists for
  CPython on Windows, and the interpreter here is whatever the environment
  happens to provide.

- **java-agent:** Restore the exec bit on the Gradle wrapper ([74f5a36](https://github.com/imbus/robotframework-PlatynUI/commit/74f5a36bc26402d4d607ac78bc45303fe16540a8))

  java/agent/gradlew was committed with mode 100644 while the byte-identical
  wrapper at apps/test-app-swing/gradlew has 100755. Both just build-java-agent
  and just test-java-agent invoke ./gradlew directly, so on Unix they failed
  with exit 126 on a fresh clone.

- **justfile:** Guard XDG_DATA_HOME against non-Linux platforms ([066fd4f](https://github.com/imbus/robotframework-PlatynUI/commit/066fd4f59ad5514b6c1856c0a6d7d50e59557933))

  The `env("HOME")` call fails on Windows where HOME is not set.
  Wrap the XDG path computation in `if os() == "linux"` so it
  short-circuits on other platforms, and add `[linux]` attributes
  to all desktop integration recipes.

- **native:** Map keyboard_known_key_names error to KeyboardError exception ([edc48e9](https://github.com/imbus/robotframework-PlatynUI/commit/edc48e960b51cab4bb035498b550f2151ea51591))

  The keyboard_known_key_names() binding was the only method mapping
  its Rust error to a generic PyException instead of the domain-specific
  KeyboardError Python exception. This made it inconsistent with all
  other keyboard operations and prevented callers from catching
  keyboard-specific errors.

- **native:** Remove ButtonLike from exports and add PointerButtonLike to __all__ ([b5bb344](https://github.com/imbus/robotframework-PlatynUI/commit/b5bb344d326e359b2531aa16248272e6bbaf8bf7))
- **native:** Simplify borrowing of PyDict in FromPyObject implementations for PointerOverridesInput and KeyboardOverridesInput ([9b47049](https://github.com/imbus/robotframework-PlatynUI/commit/9b470493fb754facd7a091170d2277223bb116cd))
- **platform:** Never look up a window by process ID 0 ([ca4a765](https://github.com/imbus/robotframework-PlatynUI/commit/ca4a76588078bfe13c30e56f0549f83a0a08156d))

  The window managers accepted 0 as a process ID. On Windows a lookup with
  0 matches any window whose process GetWindowThreadProcessId cannot name,
  so a node carrying ProcessId 0 could act on an unrelated window. Only a
  positive number counts now; a node with 0 takes the process ID of its
  ancestors, as one without any does.

- **platform-linux-wayland:** Identify the compositor without peer PIDs ([675365b](https://github.com/imbus/robotframework-PlatynUI/commit/675365b44d4c8460b399c66e70c3171d371f53ff))

  The Wayland backend recognised the PlatynUI compositor by its process:
  SO_PEERCRED on the Wayland socket, then /proc/<pid>/exe. Across a PID
  namespace boundary the kernel reports the peer as process 0, so a
  runtime in a sidecar saw an unknown compositor and lost window
  management, screenshots and highlighting, and highlight even reported
  success. Measured with the new harness: "Wayland Desktop (Unknown)", the
  Frame at {0,0,600,500} against the compositor's {10,40,600,500}, and
  "Highlighted 1 region(s)." with exit code 0.

  Identification now asks the channel every gated capability uses: one
  status request over the control socket, with a 1 s timeout of its own,
  identifies the compositor by its marker. XDG_CURRENT_DESKTOP=platynui
  comes second, the Wayland socket's peer process third (still how Mutter,
  KWin and sway are told apart) and the desktop heuristic last; a peer
  that is not visible counts as no evidence. The decision is a pure
  function over that evidence and is logged once per initialization with
  the mechanism that decided, the control socket and its handshake
  outcome, the desktop value and the peer-credential outcome. A compositor
  that reports a different Wayland socket than WAYLAND_DISPLAY is warned
  about but stays identified, since a sidecar may mount the socket at
  another path.

  SO_PEERCRED is read through nix: rustix's socket_peercred returns a
  non-zero Pid and is undefined behaviour for a peer it reports as 0.
  clippy.toml now forbids it, and the crate drops rustix's net feature.

  Highlight, screenshot and the window manager refuse an unsupported
  compositor with one message naming the capability, the compositor and
  the control socket's handshake outcome. Highlight no longer reports
  success for a highlight nobody sees; clearing still succeeds. A session
  identified by its environment whose control socket does not answer fails
  each of these with the socket's own error.

  scripts/wayland-sidecar-harness.sh runs a command in a sibling PID
  namespace of the compositor and its application, and
  `just test-wayland-pidns` drives the CLI through it; a missing
  prerequisite fails the run instead of skipping it.

- **platform-linux-x11:** Identify own windows via the X server ([b3559d4](https://github.com/imbus/robotframework-PlatynUI/commit/b3559d41964c2199726e64db7237f303b60cd6e6))

  The X11 hit-test skipped a window as the runtime's own whenever its
  _NET_WM_PID equalled std::process::id(). The two numbers come from the
  client's and the runtime's PID namespaces. Measured in a sidecar
  deployment — Xvfb, the accessibility bus and the application in one
  container, the runtime in a sibling — an application whose in-container
  PID matched the runtime's was skipped, and element-at-point answered
  "No element" over its controls.

  A window now counts as the runtime's own only when two witnesses agree:
  it reports our PID, and the X server attributes it to the same process
  as our own connection. The server's attribution is X-Resource
  QueryClientIds with LocalClientPID, derived from socket credentials; it
  is asked once per connection about our own client and cached on the
  window manager, and about a window only when that window reports our
  PID. Both answers are numbers in the server's namespace, so they compare
  wherever the server runs: the sidecar's application window is resolved,
  while the runtime's own window is still skipped where the server numbers
  it differently — WSLg, a container on the host's display, ssh -X. A
  server without X-Resource 1.2 keeps the previous comparison and warns
  once per connection; a window whose owner the server cannot name is not
  skipped. The decision and the server's view are logged once per
  connection.

  `just test-x11-pidns` builds these deployments with unshare and a real
  Xvfb and fails with the missing prerequisite named instead of skipping.

- **platform-mock:** Remove auto-registration of mock highlight and keyboard devices ([e2ee2b9](https://github.com/imbus/robotframework-PlatynUI/commit/e2ee2b933b7b386d0890fa0e51c8b5e293ba405a))
- **platform-windows:** Keep a monitor name made of whitespace ([48a08c9](https://github.com/imbus/robotframework-PlatynUI/commit/48a08c9961881f492bd89b8b600a591307c0baf5))

  A display device whose device string held only whitespace lost its
  monitor name. Like every value a platform reports, the name is now
  dropped only when it is empty.

- **platform-windows:** Record a refused foreground change at debug ([4b1fc6c](https://github.com/imbus/robotframework-PlatynUI/commit/4b1fc6c12096d0e8688e2f130a326ce98a84a02f))

  When SetForegroundWindow refused to bring a window to the foreground,
  the Win32 window manager warned. Windows can refuse that despite the
  AttachThreadInput workaround, and an activation before an action goes
  on either way, so the warning was noise in healthy runs. The
  diagnostic-logging spec asks for debug in this case; the UI Automation
  path already records it that way. The Java Access Bridge and agent
  windows activate through this window manager, and that is where the
  warning showed up in the Windows lane.

  A unit test activates the null handle, which Windows always refuses,
  and checks for one debug record and no warning. The capture helper it
  shares with the factory tests moves to its own test module.

- **platform-windows:** Use static error labels in pointer helpers ([c0938e3](https://github.com/imbus/robotframework-PlatynUI/commit/c0938e300c534e5e21fa94feeefe5ed5f03e03f8))
- **platform-windows:** Enhance keyboard input handling with AltGr and modifier key aliases ([ea07d7f](https://github.com/imbus/robotframework-PlatynUI/commit/ea07d7fa63a66eeedf107f3b9727240a342dc7a5))
- **provider-atspi:** Name an element by its Accessible.Name alone ([fdb20d8](https://github.com/imbus/robotframework-PlatynUI/commit/fdb20d88e03218d909ed5727159b8e81e8d6c0ca))

  An empty Accessible.Name was filled from the object attributes
  accessible-name, name, label or title, a heuristic no spec covered and
  no other provider has. The name is now Accessible.Name as reported, and
  empty when it is empty; the attributes stay readable in
  native:Accessible.Attributes.

- **provider-atspi:** Take attribute values as the bus reports them ([909087e](https://github.com/imbus/robotframework-PlatynUI/commit/909087e5e7519bdb5b5afc031cd5151cfb17dc67))

  AT-SPI trimmed every string it read and turned a blank one into no
  value, so an element answered @Name, @Id, @Description and its native
  attributes differently than on the other providers, and a locator
  written against one provider could fail on another. D-Bus has no null
  string, so only the empty string means unset; whitespace is part of
  the value. The Name and Id fallbacks take the first non-empty object
  attribute unmodified, and the object-attribute maps keep every key.

- **provider-atspi:** Place applications by tree level, not interface ([9ce921b](https://github.com/imbus/robotframework-PlatynUI/commit/9ce921b056f4a8201eab70a5adc94e22020bf13c))

  The provider made every node that reports the AT-SPI Application
  interface an app:Application, whatever its role, and gave it the process
  attributes and a process-derived Id. AT-SPI requires that interface only
  on an application's root, and Avalonia 12 adds it to every window: the
  window appeared as a second, nested application, and its panels were
  taken for top-level windows.

  The applications the registry lists are now the application level, with
  the role their object reports. Below that level the interface shows up
  only as native attributes. The runtime finds an element's window through
  the app namespace alone.

  Behavior changes for the release notes:
  - In Avalonia 12 applications the window is control:Frame with the
    window patterns. Its children lose the window patterns, @IsActive and
    the window-manager bounds, and @ProcessId and the app: attributes
    leave the window.
  - A node below the application level with the AT-SPI role application
    is control:Application instead of app:Application.
  - An application root whose role is not application is app:<its role>;
    no known toolkit does this. A root whose role read times out is
    app:Unknown for that enumeration.

- **provider-atspi:** Read process attributes through the shared reader ([6c85cb8](https://github.com/imbus/robotframework-PlatynUI/commit/6c85cb8916c81e15b9013f23059b1bb3c9ff51b9))

  AT-SPI read the process table on its own: the process name was the
  executable's stem or the truncated comm, the command line lost its
  arguments' whitespace, and a node held across a pid reuse could report
  the process that received its pid. It now records the process once per
  application node and reads through the shared reader.

  Behavior changes for the release notes:
  - ProcessName is the executable's full file name (python3.12, not
    python3), and absent instead of comm when the executable is unreadable.
  - CommandLine keeps every argument as it is, empty ones included.
  - A node whose process has ended, or whose pid was reused, lists no app:
    attribute.

- **provider-atspi:** Lower a non-actionable children warning to debug ([7c0dd40](https://github.com/imbus/robotframework-PlatynUI/commit/7c0dd401820410701c6742a8892446e732cbe707))

  Native warnings now reach the Robot Framework log and console, so the
  healthy lanes were reviewed for native warnings. The mock lane had
  none. The X11 and compositor lanes had one kind: "children:
  get_children failed or timed out", logged during Wait Until Exists in
  the Inspector-picker suites, where the picker's own tree update removes
  a node between its enumeration and the children call.

  A node vanishing in a live tree is expected and nothing a user can act
  on, and a call that actually timed out already warns in
  block_on_timeout. The message is now logged at debug. With it, all
  three lanes run without a native warning.

- **provider-atspi:** Warn when window bounds fall back ([446ba09](https://github.com/imbus/robotframework-PlatynUI/commit/446ba092e60b5b4f82affe73f19e4d475a2c8529))

  A real top-level window's position on screen comes from the window
  manager. When the window manager could not answer, because the window
  lookup or the bounds read failed, resolve_window_manager_bounds dropped
  the error and the node answered with the extents its toolkit reports,
  with nothing in the log. On X11 those are screen coordinates and usually
  close; on Wayland they are relative to the window. Measured in a sidecar
  deployment whose compositor went unrecognised: the Frame reported
  @Bounds {0,0,600,500} where the compositor's own geometry was
  {10,40,600,500}, and a click by locator reported success and missed by
  the window offset.

  The fallback value is kept on purpose. A best-effort rectangle is still
  a useful answer for a read, and failing instead would make every control
  in the window unreadable and unpickable. What changes is that the
  substitution is logged: a warning names the node, its bus name and
  object path, the call that failed with the window manager's error (for
  the PlatynUI compositor that error names the control socket) and the
  toolkit rectangle used instead. It fires once per top-level while that
  window's reads keep failing, keyed by bus name and object path so a
  re-enumerated tree does not repeat it, and a successful read re-arms it.
  The record lives next to the injected window manager and so has its
  per-runtime scope. Nodes inside a window and grafted popups keep their
  own geometry sources and log nothing.

  The composition of a node's extents moves into extents.rs behind a
  small trait over its geometry sources, so the rule is tested without a
  display or a bus.

- **provider-atspi:** Resolve process identity per bus connection ([45424fc](https://github.com/imbus/robotframework-PlatynUI/commit/45424fcfcfebd5b00116e6d5b19a43d78e7d1147))

  The provider recognised its own UI by comparing the process ID the bus
  daemon reports for a peer with getpid(). Across PID namespaces the two
  numbers come from different numbering spaces. Measured in a sidecar
  topology, where the application, the display server and the bus daemon
  share a namespace and the runtime runs in a sibling: an application whose
  in-namespace PID happened to equal the runtime's own was dropped as "ours",
  leaving an empty tree. On daemons that answer 0 for a peer they cannot see,
  every such application compared equal to every other. The process-table
  attributes were read with the reported number, so the application showed
  the automation binary's own name, or the attributes came back empty.

  The provider now asks the daemon once per bus connection what it reports
  for its OWN connection and compares that with getpid(): an equal number
  means the daemon numbers processes as the runtime does (local numbering),
  anything else — 0, nothing, "unknown", another number — means no identity.
  Own-UI exclusion in the tree walk and in the popup watcher happens only on
  a positive match under local numbering; 0 or an absent number is never an
  identity. Only definitive answers are cached; a timeout or a failed call is
  retried, and a peer's verdict is derived from the cached answer on every
  read, so a lookup that failed early cannot freeze it. Connections are keyed
  by the bus GUID and their unique name, and a provider that shuts down
  forgets only its own. The provider-side hit-test guard is gone: skipping
  the caller's own windows is the window system's decision.

  Behaviour change for PID-valued attributes: @ProcessId reports the number
  the daemon gives for the application's connection and is absent, never 0,
  when the daemon cannot tell; element.id follows it and is never "0". The
  app:* process-table attributes are read only through a PID valid in the
  runtime's namespace and each is reported only when it was actually read —
  no empty strings, nulls or "unknown" stand-ins. The provider no longer
  reports app:Architecture: Linux keeps no architecture per process, and the
  fallback answered the architecture the runtime was built for. An
  unreachable accessibility bus now fails with the configured address in the
  error instead of looking like an empty bus.

  `just test-atspi-pidns dbus-daemon|dbus-broker` runs a local, fail-loud
  harness that builds the sidecar, a runtime beside the daemon and a forced
  PID collision with unshare against a real bus daemon.

- **provider-atspi:** Correct bounds for embedded surfaces (Qt MDI) ([71ee056](https://github.com/imbus/robotframework-PlatynUI/commit/71ee056049657dc54c861c514d4a9c42dc661dbd))

  Identify real platform top-level windows by checking the parent's
  namespace (`Application`) instead of the role string, and resolve
  non-toplevel extents by walking the parent chain via `CoordType::Parent`.
  Qt treats `QMdiSubWindow` (and similar embedded widgets) as window
  boundaries for `CoordType::Window`, so the previous "outer top-level
  bounds + window-relative coords" math returned wrong positions for
  anything inside an MDI area.

- **provider-jab:** Parse value strings without trimming them ([d9d1d7f](https://github.com/imbus/robotframework-PlatynUI/commit/d9d1d7f45b6a9334342dc14d413316647defeb07))

  The bridge sends an AccessibleValue, a java.lang.Number, as its
  toString(), so the value stays a number. Both readers trimmed the
  string first and returned the trimmed string when it was not numeric,
  against their own comment. They now share one function that parses the
  string as reported, keeps any other string unchanged, and treats the
  empty string or a failed call as no value.

- **provider-jab:** Keep live-fixture lint attributes ahead of cfg gate ([263a6b7](https://github.com/imbus/robotframework-PlatynUI/commit/263a6b750a60a3ae78c44640debe939d6b896249))

  On non-Windows targets the crate-level #![cfg(windows)] strips every
  inner attribute that follows it, so the doc_markdown allow never
  applied and all dev-dependencies of the test target counted as unused,
  failing the Linux clippy lane with -D warnings.

  Move the allows above the cfg, add a not(windows) allow for
  unused_crate_dependencies, and drop the doc_markdown allow entirely
  (backticking the one offending word instead).

- **provider-java:** Name agent elements by their accessible name ([0697925](https://github.com/imbus/robotframework-PlatynUI/commit/06979251057bd08dadc44c5ab6741ef4e65d5e06))

  The agent let a component's developer name, a window's title, a cell's
  model value and a header's value stand in for the accessible name, so a
  Swing dialog named its controls by their ids and the same application
  answered @Name differently through the agent and the bridge. Name is
  now Swing's accessible name for every element; the other values stay
  readable as Id, native:ComponentName, native:WindowTitle and the typed
  native:TableCell.ModelValue. Text comes from every AccessibleText Swing
  provides, which for plain labels, buttons and renderers is none.

- **provider-java:** Scope agent runtime ids per view ([22f8cf0](https://github.com/imbus/robotframework-PlatynUI/commit/22f8cf0f3e994742d3605a18d5a64cdeef8247a4))

  The agent backend shows a Java window twice: flat under the desktop,
  and under its process's app:Application node. Each view is now a node
  of its own with a runtime id of its own, as with UI Automation and the
  Java Access Bridge. The view is fixed where a node is created and
  passed to every child; a hit-test result belongs to the application
  view.

  New ids in the application view: agent/app/<pid>/<element id>. The
  flat view keeps agent/<pid>/<element id>. SelectedItems names the
  selected children in the view of the node that reports them.

- **provider-java:** Make the tracing-subscriber dev-dep Windows-only ([0c401a3](https://github.com/imbus/robotframework-PlatynUI/commit/0c401a3e3ba9105c419416a7f11b5c5450ece462))

  The crate body is `#[cfg(windows)]`, so on every other target the lib test
  compiles to nothing and the dev-dependency is unused. With
  `unused_crate_dependencies` denied through `-D warnings` that fails the
  workspace clippy gate on Linux, in `just clippy` and in CI.

  `tests/live_fixture.rs` already allows the lint for its own non-Windows case;
  the lib test has no such allowance, so the dependency moves to the target
  table where the test that uses it already lives.

- **provider-windows-uia:** Derive runtime ids UIA does not report ([4dc9bf4](https://github.com/imbus/robotframework-PlatynUI/commit/4dc9bf44e12654cbbd575915e7cf9103af381f5e))

  UI Automation reports an empty runtime id for MSAA-proxied elements
  without a window of their own, such as the tab headers of a WinForms
  TabControl on .NET Framework. The provider turned each of them into the
  same uia://app/<pid>/, and a failed read into the shared uia://temp,
  so they all counted as one node.

  Such an element now gets an id derived from its parent's: /child:<n>
  from its MSAA child id, which UI Automation's own CompareElements
  identity rests on, otherwise /index:<n> from its raw-view position.
  Both are reproducible, so a listed and a hit-tested element carry the
  same id. A node without a parent node walks up the raw view in a loop,
  at most 256 ancestors. When no id can be derived, the element gets a
  unique /unlisted:<n>. Ids that UI Automation reports stay as they are.

  apps/winforms-test-window is a PowerShell helper that shows such a tab
  control on Windows PowerShell 5.1, with no build. The suite
  tests/acceptance/winforms checks the derived ids in the real-windows
  lane. The OpenSpec change records the verification against KeePass.

- **provider-windows-uia:** List native properties that are empty ([05fb249](https://github.com/imbus/robotframework-PlatynUI/commit/05fb249a21cde1a63fa2dd79dd03e061281c77f4))

  The variant conversion dropped an empty BSTR, so a property UIA
  reported as the empty string was missing from native listings,
  snapshots and the Inspector, although native attributes show what the
  platform reported. control:Description keeps its own rule and stays
  absent when empty.

- **provider-windows-uia:** Replace Lazy with LazyLock in static descriptors ([ee7e508](https://github.com/imbus/robotframework-PlatynUI/commit/ee7e50864818f59301ddf703c78f92aa2c29a23c))

  The Linux refactoring missed two Lazy → LazyLock replacements in
  provider.rs, causing a build failure on Windows. The import for
  std::sync::LazyLock was already present.

- **provider-windows-uia:** Update UiaNode's availability check to use CurrentProcessId ([500b4a2](https://github.com/imbus/robotframework-PlatynUI/commit/500b4a2a8141e01af19f3b755c537bdb4ea3def0))
- **provider-windows-uia:** Correct getting clickable point and bounding rectangle ([1d4a42a](https://github.com/imbus/robotframework-PlatynUI/commit/1d4a42add4d3966cfd545512a2f884fd9adc1f1b))
- **providers:** Report Id only as the toolkit's identifier ([2db1cab](https://github.com/imbus/robotframework-PlatynUI/commit/2db1cabc23ca088ce8ada2476d1261e7d3aceb89))

  Id is the identifier a toolkit reports for automation, but AT-SPI made
  an application's process ID its id() and listed @Id="" on every element
  without one, and UIA used the process name as an application's Id. The
  accessor and the attribute could disagree, and [@Id] held for every
  AT-SPI node. An application is already identified by @ProcessId on
  every provider.

  Behavior changes for the release notes:
  - element.id / ${el.id} of an application is None; on AT-SPI it was
    the process ID, on UIA the process name. @ProcessId stays the way to
    identify an application.
  - UIA application nodes no longer carry @Id, so
    app:Application[@Id="..."] stops matching; @Name and
    @app:ProcessName still match.
  - One-line descriptions of applications lose their #... suffix, in
    BareMetal action lines, log records and describe().
  - On AT-SPI, a node without an accessible-id no longer lists @Id="":
    [@Id] holds only with an id, [@Id=""] stops matching, Get Attribute
    ... Id fails with "attribute not found", and snapshots, platynui-cli
    query and the Inspector lose the empty Id lines.
  - On AT-SPI, an application root's own accessible-id, such as Qt's
    QApplication, is no longer its @Id; it stays readable as
    native:Accessible.AccessibleId.

- **providers:** Report process attributes by one contract ([bfd9297](https://github.com/imbus/robotframework-PlatynUI/commit/bfd92975aed564ab3219c933544ed4c9b5088beb))

  UIA, JAB and the Java agent read an application's process attributes
  through the shared reader now, so they report one process the same way.
  Before, a value they could not read came back as "", a null or
  "unknown", the architecture fell back to the host's or the build's, and
  the agent reported under control: what the JVM said about itself.

  Behavior changes for the release notes:
  - An attribute that cannot be read is absent, not "" or null.
  - A node whose process has ended, or whose pid was reused, lists no app:
    attribute; ProcessId 0 is never listed, attached to or hit-tested.
  - Java agent nodes carry the attributes under app: and describe the JVM
    process: javaw, the launcher started, the verbatim command line and
    DOMAIN\user; StartTime is there on Java 8 too.
  - StartTime drops its milliseconds; Architecture comes from the OS, and
    32-bit ARM is arm.
  - JAB's UserName gains the domain, its CommandLine is verbatim, and node
    names drop only a trailing .exe.
  - A UIA application node whose name cannot be read now carries
    ProcessId, Technology and SupportedPatterns.

  apps/win32-test-window gives the lane a 32-bit process; the lane and CI
  build it for i686-pc-windows-msvc.

- **providers:** Mark an application node invalid once its process ends ([94f5f97](https://github.com/imbus/robotframework-PlatynUI/commit/94f5f979ccc70d76c4c3808a55aeacdf87d5941c))

  A root pinned to /app:Application[@ProcessId=...] was reused after its
  application had ended, because application nodes always answered
  valid. A query under it then waited out its timeout and failed as if
  the element were missing. The application nodes of UI Automation, the
  Access Bridge and the Java agent now answer from the process identity,
  the agent's also from whether its session is closed or degraded.

  Behavior change: a query under an application root whose process has
  ended now fails with RootNotFoundError, naming the root, instead of
  ElementNotFoundError after the timeout.

- **providers:** Stop using process ids as document-order keys ([04d3aed](https://github.com/imbus/robotframework-PlatynUI/commit/04d3aed43b81d746662908e9cab18ad870302dc1))

  The XPath engine orders two nodes by their keys when both have one, so a
  key must follow document order across the whole tree. The application
  nodes of UI Automation, the Java Access Bridge and the Java agent used
  their process id, which follows nothing: results mixing application
  nodes and windows came out of order, and Rust's sort can panic on a
  comparison that is not a total order. The contract of doc_order_key now
  says so, the providers return no key, and a testkit check proves the
  keys of the mock, the application nodes and the Java providers' top
  level against document order.

- **providers:** Keep the parent of every listed node alive ([21f9429](https://github.com/imbus/robotframework-PlatynUI/commit/21f9429e6dbf51a531074d0dc433d8bb63169760))

  Every real provider kept a node's parent only as a Weak, so a node that
  a query or children() handed out kept its ancestors only while its
  XPath snapshot leaked. Once snapshots are released, a held element
  would lose the way to its window: no activation, no parent(), no
  ancestors, no AT-SPI bounds. The child iterators of UI Automation, the
  Java Access Bridge, the Java agent and AT-SPI now hold each listed
  node's parent strongly; nodes never hold their children, and top-level
  nodes keep the desktop only weakly. UiNode::parent documents the rule,
  and the testkit checks prove it against UI Automation and, in the live
  Java tests, against the bridge and the agent.

- **python:** Keep Locator index and position counting all descendants ([9d2bc04](https://github.com/imbus/robotframework-PlatynUI/commit/9d2bc0468927730decaa407f2b879666e61fb09a))

  With positional predicates counting per step, .//Button[2] is the
  second button of every parent. On the descendants scope, a locator
  with an index, a position or raw predicates now renders one
  descendant:: step, such as descendant::Button[2], so index and
  position keep counting over all descendants in document order.

- **python:** Read Application's process from the attribute contract ([ca499f1](https://github.com/imbus/robotframework-PlatynUI/commit/ca499f1708e176d6cbe8e1a5ef1dab247846205c))

  Application.process_id read app:ProcessId, which no provider reports, so
  it always failed; process_name failed on every application the Java
  agent serves. Both read the contract's attributes now and answer None
  when the node reports none. The mock's application models process
  attributes, with the command line left out on purpose, so that listing
  and predicate agree for a missing attribute can be tested.

  Behavior changes for the release notes:
  - Application.process_id reads control:ProcessId and returns the pid.
  - process_id and process_name return None for an absent attribute
    instead of raising.

- **python:** Accept None in set_root to reset the root to desktop ([c9c30d5](https://github.com/imbus/robotframework-PlatynUI/commit/c9c30d599bef740e1bbf48b5bf3cd4d420c3e68d))
- **python:** Avoid platform-specific mypy failures ([bf93105](https://github.com/imbus/robotframework-PlatynUI/commit/bf93105e595989bf398035fe66be83b2356bc6b9))
- **python-bindings:** Restrict pointer origin to desktop|Point|Rect and update docs/README ([a5f2554](https://github.com/imbus/robotframework-PlatynUI/commit/a5f2554737e0085bf363378f5f0bfb5fdc9ae21c))

  - Remove tuple/dict acceptance for origin in platynui_native.runtime

  - Typing stays strict (runtime.pyi already limited)

  - Align docs and README example to core.Point instead of tuple

- **release:** Keep breaking changes within 0.x for bumps and dev builds ([9416a38](https://github.com/imbus/robotframework-PlatynUI/commit/9416a386b14d159c9f5fb142c37831a175bd4801))

  A BREAKING CHANGE footer previously escalated the next version to 1.0.0
  on both release paths. Enable commitizen's major_version_zero so cz bump
  caps at a minor bump while on 0.x, and mirror the same policy in the
  dev-build version script by reading that setting via tomllib.

  Also fix pre-existing defects in the dev-build bump detection: the
  patch-type check interpolated the whole list and looped over the wrong
  list, so fix/docs/perf never produced a patch bump; and commit messages
  typed as str | bytes are now decoded to str.

- **robot:** Clean up keyword source fallback ([8ccda04](https://github.com/imbus/robotframework-PlatynUI/commit/8ccda044f9bffe1461bc91ebf0ab4abf2aab027b))

  Avoid returning invalid line-only source strings and delegate unknown
  source metadata cleanly to DynamicCore.

  Keep the fallback logic minimal while preserving the expected
  str-or-none contract for static analysis.

- **runtime:** Read no list twice when sorting attributes ([eda8e93](https://github.com/imbus/robotframework-PlatynUI/commit/eda8e93e8a32b07dda189bc258def9cff4faedf9))

  An attribute links weakly to the element wrapper that listed it, and
  parent() returns that wrapper while it lives. Sorting attributes into
  document order therefore reuses the lists of children that wrapper has
  read, instead of building the element anew and reading its parent's
  list a second time in one query. The link is weak, so the wrappers of
  a snapshot stay free of strong cycles.

  RuntimeXdmNode also implements the identity hint, a hash of what its
  equality compares, so removing duplicate nodes takes constant time.

- **runtime:** Release XPath snapshots ([f171368](https://github.com/imbus/robotframework-PlatynUI/commit/f171368e7f11876cf529ae3989207d8e24be22cc))

  A child's wrapper held its parent's wrapper strongly while the parent's
  list of children held the child, so no snapshot was ever freed: every
  discarded snapshot, every query without a cache and every revalidated
  list leaked with the provider nodes it held (14.9 MiB per discarded
  snapshot of a large VS Code window). Children now link to their parent
  weakly, and a snapshot is released without recursion and outside the
  locks when it ends, and before the providers shut down. A held node
  keeps only its own ancestors instead of the whole snapshot, and the
  ancestors built upward from a held element are revalidated too.

- **runtime:** Activating a window no longer un-maximizes it ([1312085](https://github.com/imbus/robotframework-PlatynUI/commit/13120857b1c210af195447a03830d0ea85e8a1ba))
- **runtime:** Add Drop impl to KeyboardEngine for session cleanup ([65fc1d8](https://github.com/imbus/robotframework-PlatynUI/commit/65fc1d85d4943ac090e5fc9e984abd3bd47ca48e))
- **runtime:** Skip global platform init for full overrides ([db6336f](https://github.com/imbus/robotframework-PlatynUI/commit/db6336fd0cc0d8cdf2ca845cb8a6e37cfd54ac1f))

  Avoid initializing global platform modules when a Runtime is created
  with a complete set of explicit platform overrides.

  This keeps mock and test runtimes isolated from host platform setup and
  adds regression coverage for the fully overridden path.

- **runtime:** Move xpath cache ownership into runtime ([0614d6d](https://github.com/imbus/robotframework-PlatynUI/commit/0614d6d37afb71da20e76ad9a15c4de955b36c13))

  Make XdmCache thread-safe so each Runtime owns exactly one shared XPath
  cache across all threads.

  Remove the Python thread-local cache workaround, restore clear_cache()
  semantics through the runtime itself, and add regression coverage for
  cross-thread runtime access.

- **runtime:** Isolate platform lifecycle and make native runtime thread-safe ([9e85f46](https://github.com/imbus/robotframework-PlatynUI/commit/9e85f46ea4bb6dc3c4859d0e4293dfee94448c10))

  Keep platform modules alive until the last Runtime is released so one
  runtime shutdown cannot break another live instance.

  Wrap the native Python runtime in a mutex and remove thread-local XPath
  cache state so the same runtime can be used safely across threads.

  Add regression coverage for platform module lifetime and rerun the full
  Rust and Python test suites.

- **runtime:** Update FromPyObject implementations for PointerOverridesInput and KeyboardOverridesInput to match new PyO3 ([54e4bfc](https://github.com/imbus/robotframework-PlatynUI/commit/54e4bfc3057d069922ba43a1f095b5611bdcc031))
- **scripts:** Make session runtime dirs writable before removal ([7ebcae6](https://github.com/imbus/robotframework-PlatynUI/commit/7ebcae61986695209331de954b3438551ca8eb16))

  Services activated on the isolated session bus may stage read-only
  trees in the private XDG_RUNTIME_DIR (e.g. the dfiles portal backend
  copies its QML files into r-x directories). rm -rf cannot unlink
  entries of a read-only directory, so cleanup printed a permission
  error per file and left the whole session dir behind in /run/user.

  Make the tree user-writable before removing it in all four session
  scripts that share this cleanup.

- **scripts:** Start the AT-SPI bus launcher without --a11y=1 ([5f48e76](https://github.com/imbus/robotframework-PlatynUI/commit/5f48e76422e9a51264b135c64b859b2c59684ddd))

  at-spi2-core 2.62 dropped the launcher's --a11y= and --screen-reader=
  options, and the launcher now aborts on them. Every session script passed
  --a11y=1, so no session got an accessibility bus and every acceptance suite
  failed at once. The option only forced accessibility on, which
  --launch-immediately and linux-a11y-enable.sh already take care of.

- **scripts:** Tear down lane sessions via transient systemd scopes ([1962f5f](https://github.com/imbus/robotframework-PlatynUI/commit/1962f5f69a2046d6e049160fb7fe4b3a911049cd))
- **scripts:** Resolve AT-SPI helper binaries across distro layouts ([8cac2c6](https://github.com/imbus/robotframework-PlatynUI/commit/8cac2c6aa3e46c72663269cfc2fa63ab5367c44d))

  The session scripts launched at-spi-bus-launcher and at-spi2-registryd from a
  hardcoded /usr/lib path. On Debian/Ubuntu (the CI runners) these binaries live in
  /usr/libexec, so the explicit launch failed with 'No such file or directory' and
  only D-Bus auto-activation kept the bus alive. Resolve the binaries across the
  common distro layouts (/usr/libexec, /usr/lib, arch-specific dirs) with a PATH
  fallback, in both startxsession.sh and the shared setup-atspi.sh.

- **scripts:** Update regex pattern for robotframework-platynui version matching ([d0b0e41](https://github.com/imbus/robotframework-PlatynUI/commit/d0b0e414c56524a14abbdeb3e0dd87526a266467))
- **simple-node:** Assign document order for standalone trees ([4db05fd](https://github.com/imbus/robotframework-PlatynUI/commit/4db05fd6e5d1688f944a9896648d098ff2f3d191))
- **test:** Keep the pidns tests lint-clean on Windows ([d1d521b](https://github.com/imbus/robotframework-PlatynUI/commit/d1d521b6eaa9442585fc5040f1e5bfac28d22639))

  `just clippy` failed on Windows: a false crate-level `cfg` drops every
  attribute after it, so the Linux-only PID-namespace tests lost their
  `allow(unused_crate_dependencies)` and the empty crate tripped the lint.
  Neither CI nor the Linux cross-checks build these targets for Windows,
  so only a local Windows run shows it.

  The `rustix` disallowed-methods entry cannot resolve on Windows either,
  and `test-compositor-pidns` was the only pidns recipe not Linux-only.

- **test:** Run the X11 acceptance lane headless on Xvfb, shared --backend ([a32d975](https://github.com/imbus/robotframework-PlatynUI/commit/a32d975ab148d68da08149d6c3d72db00cac5f29))

  The X11 acceptance lane failed on CI: at-spi2-registryd reported "Cannot open
  default display" / "Authorization required, but no authorization protocol
  specified". Cause: the Xephyr-in-Xvfb nesting (via xvfb-run) together with the
  session's `unset XAUTHORITY` left clients with no valid X authorization. Headless
  X11 does not need a nested Xephyr — run directly on a standalone Xvfb.

  - startxsession.sh: add a `--backend auto|nested|headless` parameter, mirroring
    startcompositor.sh. `headless` starts a standalone Xvfb with -ac (no xauth
    cookie needed); `nested` keeps Xephyr inside the host display for local dev;
    `auto` (default) picks nested when a DISPLAY is present, else headless.
    winit/xephyr are accepted as aliases for nested.
  - startcompositor.sh: accept the same auto/nested vocabulary (winit kept as an
    alias) so both scripts share the --backend surface.
  - justfile: test-acceptance-x11 now passes `--backend headless` instead of
    wrapping the run in xvfb-run — symmetric with test-acceptance-compositor.

  Verified locally: headless (Xvfb) 17/17 with no X-auth error, and nested (Xephyr)
  17/17 in a visible window. The compositor-screenshot failure on GPU-less runners
  is a separate issue (offscreen renderer needs a DRI render node) — next session.

- **test-app-egui:** Start without stored window state ([673be94](https://github.com/imbus/robotframework-PlatynUI/commit/673be948c74436a15ba0f2a7094aa7ceb36ee76e))

  On Windows the fixture restored the geometry of an earlier run. The
  Windows lane builds it together with the Inspector, which switches on
  eframe's persistence feature for both, and eframe then loads the window
  size and egui memory from %APPDATA%\<app-id>\data\app.ron. A suite that
  had shrunk the default instance left 640x480 behind, every later
  instance started that small, and the radio buttons fell below the
  window's edge, so clicks landed on the desktop.

  eframe's persist switches only stop the writes, so the fixture also
  points its storage at a file that nothing writes. It now starts at its
  own size whatever an earlier run stored, and stores nothing itself.

- **test-app-egui:** Give the button group its bounds ([cbf8e1b](https://github.com/imbus/robotframework-PlatynUI/commit/cbf8e1b885ef7c9e7937e2160e506ee0f665c6d8))

  On the Wayland lane every widget in the new button group was placed at
  window-local coordinates, so clicks and hit tests missed them. egui
  gives its containers no bounds, and AT-SPI places a node relative to
  its parent: without the group's own extents the provider fell back to
  the node's screen extents, which a Wayland client reports relative to
  its window. X11 reports real screen coordinates and hid it.

- **tests:** Make the Qt acceptance suite run on Windows ([923be37](https://github.com/imbus/robotframework-PlatynUI/commit/923be37c45a57c39620d3e0cf6a0037f1e8cee05))

  Two Windows-only blockers kept the Qt (PySide6) acceptance suites from
  running:

  1. The interpreter path was relative (.venv/Scripts/python.exe), which
     fails CreateProcess with FileNotFoundError. Use absolute, OS-native
     paths (mirrors the egui lane).

  2. The project venv's python.exe is a uv trampoline: launching it spawns
     the real interpreter as a child with a different PID, so the tests'
     /app:Application[@ProcessId=...] window-pinning found nothing (and
     teardown orphaned the child). Launch the BASE interpreter directly and
     redirect it into the venv via __PYVENV_LAUNCHER__ (the mechanism the
     trampoline itself uses), so the launched PID owns the window.

  The __PYVENV_LAUNCHER__ redirect is Windows-scoped: QT_APP_PYVENV_LAUNCHER
  is empty on Linux/macOS (venv python is a symlink, no trampoline), where
  the launcher takes the plain branch. The UIA locators were already correct
  (accessibleName surfaces as @Name), so no locator changes were needed.

- **tests:** Narrow mypy ignore to [call-arg] in test_settings ([0445261](https://github.com/imbus/robotframework-PlatynUI/commit/044526148bc07d4216f83fda24f2581c539f5e4e))

  The newer mypy (from the uv.lock refresh) reports the kw-only positional
  call Settings(1.0) as [call-arg], making the old [misc] ignore unused.

- **tests:** Update test-python to build native package with mock-provider first ([9c35081](https://github.com/imbus/robotframework-PlatynUI/commit/9c35081cae2e53ccac9ca388a7fb599755d9e2a6))
- **tests:** Update error message to use English for empty keyboard sequence ([e7a4100](https://github.com/imbus/robotframework-PlatynUI/commit/e7a41002768b80b83b139263d409d591918a09ee))
- **tests:** Add unused braces allowance for root and sc fixtures ([7170358](https://github.com/imbus/robotframework-PlatynUI/commit/717035832152f1fcd29363690e523878d6007fe3))
- **uia:** Free the runtime-id array of every element ([f5569db](https://github.com/imbus/robotframework-PlatynUI/commit/f5569db73b35d2c6346d51c1024c2cac869df4a6))
- **uia:** Serialize the UIA library's first initialization ([8d43551](https://github.com/imbus/robotframework-PlatynUI/commit/8d435517895c1f153036efcbe96f93b4b566226d))

  Eight threads whose first ever UIA use overlaps leave seven of them with
  GetRootElement returning E_FAIL. CoCreateInstance(CUIAutomation) succeeds
  on every one of them, but the client library is not actually usable until
  one real call has completed, and racing that first call breaks all but
  the winner. Once any thread is through, the library stays warm for the
  process lifetime and concurrent use is fine.

  uia() now takes a process-wide lock for the creation path and, on first
  arrival only, performs one GetRootElement() to finish the initialization.
  Both halves carry weight: serializing CoCreateInstance alone only takes
  the cold-start failures from 7/8 down to 1-2/8. Each thread still keeps
  its own instance in the existing thread-local, so the lock is taken once
  per thread rather than per call.

  This is reachable in production, not just under a test harness — the
  provider is entered from several threads (tree streaming, the Inspector,
  hit-testing) and a cold parallel start is the normal way a process begins.
  It stayed hidden because it can only ever happen on the very first access.

  The regression test fans out eight threads on a cold process and must
  stay the only test in its module: once the library is warm, a fan-out
  proves nothing.

- **wayland:** Correct window bounds for GTK4 CSD windows ([5397215](https://github.com/imbus/robotframework-PlatynUI/commit/5397215ebffa37747f002085e7b9db3cb652ddc7))

  GTK4 CSD windows include an invisible shadow border in their surface
  buffer. The compositor places the surface at the buffer origin, but
  AT-SPI reports coordinates relative to the content area (inside the
  shadow). This mismatch caused highlight frames and element positions
  to be offset by the shadow size (~25px).

  Compositor (wayland-compositor):
  - Report geometry offset, decoration mode and opaque region in the
    IPC window info response

  Platform (platform-linux-wayland):
  - Compute CSD shadow offset from opaque region with geometry fallback
    using min(geometry, opaque) per axis to avoid over-correction from
    rounded corners
  - Only apply the correction for GTK4 CSD windows; other toolkits
    (Qt, etc.) report correct AT-SPI extents even with CSD

  Provider (provider-atspi):
  - Resolve toolkit identity (name + major version) from AT-SPI
    Application.ToolkitName and Application.Version, cached per bus name
  - Pass toolkit hint through WindowManager::bounds() so the platform
    can apply toolkit-specific adjustments

  Core (WindowManager trait):
  - Add toolkit_hint parameter to bounds() for toolkit-aware corrections

- **wayland:** Join event loop thread on shutdown ([5f8f09d](https://github.com/imbus/robotframework-PlatynUI/commit/5f8f09d50abeaf8a68e7ec769a7168385490cd01))

  Store the Wayland event loop thread handle in global state and join it
  during shutdown and reinitialization.

  This prevents stale background updates from racing with cleared or
  recreated global desktop state and makes the platform lifecycle
  deterministic.

- **wayland:** Route pointer events to popups extending beyond SSD window bounds ([ac478fb](https://github.com/imbus/robotframework-PlatynUI/commit/ac478fb28c43332bccb254b02da3d420d7df9e53))

  SSD windows used only client geometry + decoration borders for hit-testing,
  so XDG popups (submenus, dropdowns) that extended beyond the parent window
  were visible but unreachable by the pointer.

  Now pointer_hit_test() also checks bbox_with_popups() via Space::element_bbox(),
  falling through to ClientArea when the cursor is over a popup outside the SSD
  zone. Fixes Kate and other Qt/GTK apps with cascading menus.

- **wayland-compositor:** Remove the control socket on exit ([20678a9](https://github.com/imbus/robotframework-PlatynUI/commit/20678a922e12750f72532ad8d2dbf0e7284546b3))

  The compositor created $XDG_RUNTIME_DIR/<socket-name>.control at start-up
  but never removed it, so every run under a unique socket name left a
  file behind: about thirty per run of the IPC tests, one per run of the
  PID-namespace test.

  setup_control_socket now returns a guard that State holds and that
  removes the file when the compositor exits, on every regular path:
  the shutdown command, SIGTERM/SIGINT, the watchdog and
  --exit-with-child. It removes the file only while the path still names
  the socket this compositor bound (same device and inode), so a
  compositor that has since taken the path over keeps its socket. A
  compositor killed with SIGKILL still leaves the file, which the next
  start under the same name replaces, as before.

- **wayland-compositor:** Stop busy-spinning the winit event pump ([ee0167e](https://github.com/imbus/robotframework-PlatynUI/commit/ee0167ec38f9556b73c1e117101fdb5f5efc8936))

  Register `WinitEventLoop` directly with calloop instead of self-pinging
  on every `PumpStatus::Continue`.  Smithay's `WinitEventLoop` already
  implements `EventSource` and exposes its internal wake fd, so calloop
  can sleep on epoll between events.  The previous self-ping pattern had
  the calloop dispatching the ping callback, which called
  `pump_app_events(Duration::ZERO)` (non-blocking, returns Continue
  immediately when no events) and re-pinged itself — burning a full core
  even when nothing was happening.

  Per-iteration housekeeping (`space.refresh`, popup cleanup, output
  reconfiguration, client flush) moves into the `event_loop.run` idle
  hook, matching the headless backend's structure.

- **wayland-compositor:** Update clipboard focus on keyboard focus change ([331a4f6](https://github.com/imbus/robotframework-PlatynUI/commit/331a4f6b788fe4e20dcf7cbd80a6fc67abc21edd))

  Call set_data_device_focus() and set_primary_focus() in
  SeatHandler::focus_changed() so newly focused clients receive
  wl_data_device.selection events with the current clipboard content.
  Without this, cross-application copy/paste (e.g. Kate → GNOME Text
  Editor) silently fails.

- **wayland-compositor:** Activate protocol handlers and add diagnostics tracing ([19c9993](https://github.com/imbus/robotframework-PlatynUI/commit/19c999314f1c03c866e3415caa02483a9d071db3))

  Protocol audit revealed two handlers that silently accepted requests
  without sending required activation events:

  - keyboard_shortcuts_inhibit: call inhibitor.activate() so VNC/RDP
    viewers receive the `active` event
  - pointer_constraints: call constraint.activate() via
    with_pointer_constraint() so clients receive `locked`/`confined`

  Add structured tracing (debug/warn) to decoration, xdg_activation,
  xdg_dialog, and seat focus_changed handlers for better diagnostics.

- **wayland-compositor:** Correct X11 popup positioning and menu dismissal ([f93072a](https://github.com/imbus/robotframework-PlatynUI/commit/f93072a01421acd762d721fd48f1a0ffb06e6779))

  Override-redirect windows (menus, tooltips, dropdowns) opened by X11/XWayland
  clients suffered from three positioning and lifecycle issues under SSD:

  1. Popups appeared at wrong screen positions because
     mapped_override_redirect_window used cascade placement instead of
     honouring the X11 window's own geometry.

  2. After an interactive move or resize, popups still appeared at the old
     position because the compositor never informed the X11 client of its
     new coordinates via configure().

  3. Clicking on SSD decorations, the desktop background, or another
     window's titlebar did not dismiss open X11 menus because those areas
     have no wl_surface — the X11 pointer grab never received the
     outside-click that toolkits need to exit their menu loop.

- **window-manager:** Pick the right window when a11y name != title ([9cf804f](https://github.com/imbus/robotframework-PlatynUI/commit/9cf804f877445ba897c8b9b12e2673159e1c3a1f))

  Windows sharing a process were disambiguated by matching the AT-SPI accessible
  name against the native window title. When they diverge (e.g. a Qt dialog whose
  accessibleName differs from its windowTitle) the match failed and another
  window's bounds were returned — typically the main window's.

  - X11 (EWMH): correlate a candidate by the node's AT-SPI screen extents instead
    of guessing, and error out rather than falling back to the first candidate.
  - Wayland (compositor IPC): the same title-only match failed outright, so
    resolve_window returned nothing and bounds/move/activate silently degraded.
    Add a size-based fallback (the AT-SPI size is correct on Wayland even though
    the position is not), stored on the selector so refreshes stay correct.

  Add a PySide6 Qt test app (apps/test-app-qt) that reproduces the divergence and
  the tests/acceptance/qt suite (bounds, interaction, modal) that guards the fix
  on X11, the Wayland compositor and Windows. PySide6 is a dev dependency; the app
  is excluded from the Cargo workspace and covered by ruff and mypy.

- **windows-uia:** Avoid 10s OLEACC stall when finding launched windows ([3a805c7](https://github.com/imbus/robotframework-PlatynUI/commit/3a805c700c91349931efc1dfb6e554621c633b54))

  Desktop and per-application window enumeration walked UIA's RawView tree
  from the desktop root. Materialising a top-level window without a ready
  native UIA provider goes through the MSAA-to-UIA bridge
  (AccessibleObjectFromWindow), which blocks on OLEACC's fixed ~10s
  WM_GETOBJECT SendMessageTimeout. A freshly launched AccessKit/egui app
  therefore stalled every app-window query for ~10s until its provider
  registered.

  Enumerate top-level windows with EnumWindows + ElementFromHandle and gate
  each on readiness (a native UIA provider, or a bounded WM_GETOBJECT
  probe), so a not-yet-ready window is skipped for the current pass instead
  of stalling. Restrict to real top-level windows (visible, not
  WS_EX_NOACTIVATE, not cloaked) to match UIA's desktop view, and share the
  enumeration between the desktop children (ElementAndAppIter) and one
  application's windows (AppWindowIter, filtered by process id) so both
  stay consistent.

  Also make Activate Window wait (bounded) until the target is actually the
  foreground window; SetFocus is asynchronous and the now-fast lookup would
  otherwise race the activation.

- **windows-uia:** Resolve app windows on demand ([b2e44d7](https://github.com/imbus/robotframework-PlatynUI/commit/b2e44d7d0eb1fe6484debb433b38c100fcc2bdd7))

  Remove the retained per-process window buckets from synthetic application nodes and resolve app windows from UIA when children are requested.

  Keep root desktop children in provider order, then stream synthetic application nodes after the top-level elements.

- **windows/highlight:** Check DC/DIB creation and clean up on failure ([d5a3413](https://github.com/imbus/robotframework-PlatynUI/commit/d5a34134b7aa70f4c652a9f33857ee1e6e3b3be3))

  - Guard  and  results; early return.
  - Replace  expect with safe match; free DCs on error paths.
  - Import /; no change to drawing logic.
  - Build and clippy stay clean (no warnings).

- **xkb-util:** Resolve missing keys when using secondary keyboard layouts ([1756589](https://github.com/imbus/robotframework-PlatynUI/commit/1756589c382466118f8318cd6479f68e24e45e6e))

  Keys like Space, Enter, and Tab that define fewer XKB groups than the
  total keymap were silently skipped when scanning a secondary layout.
  This caused basic input to fail after switching to e.g. English in a
  de,us multi-layout configuration.

  Apply XKB-conformant group clamping so keys with fewer groups fall
  back to their last available group instead of being omitted entirely.

  - Improve keyboard error messages to include the active layout name
    (e.g. "English (US)") and the backend in use (EIS, virtual-input,
    control-socket), making it clear why a character cannot be typed
  - Expose the layout name from the keymap lookup so backends can
    report it in diagnostics

- **xkb-util:** Gate crate for Linux only to avoid Windows/macOS build failures ([f2b8090](https://github.com/imbus/robotframework-PlatynUI/commit/f2b8090ad4ed96da353518607e1be60bb476f956))

  xkbcommon is only available on Linux. Move dependency to
  `[target.'cfg(target_os = "linux")'.dependencies]` and wrap all
  public API re-exports with `#[cfg(target_os = "linux")]`.
  The crate compiles as empty on non-Linux targets.
  Consistent with eis-test-client which already gates its
  platynui-xkb-util dependency on Linux.

- **xpath:** End sibling walks over nodes that share an identity ([d3d32a1](https://github.com/imbus/robotframework-PlatynUI/commit/d3d32a167b4cda462e591fa379d704e83b6fa7c6))

  The engine found a node's next or previous sibling by looking the node
  up in its parent's list by equality. Runtime elements are equal when
  their runtime ids are, so two siblings that a provider reports with one
  id sent child, descendant, following and sibling steps back to the same
  sibling forever. KeePass's WinForms tab headers do this through UI
  Automation, and every query that walked past them hung.

  - XdmNode gains sibling_position(): the node's place in its parent's
    list when the model knows it. The engine moves to the entry next to
    it, checked by one equality test, and looks the node up only without
    a position. The runtime adapter records the place of every element
    it lists, for every provider, and indexes its shared list directly.
  - The cancel flag is checked for every candidate a walk visits, so a
    wildcard walk that count() consumes stops too.

  Engine and runtime tests cover equal siblings and cancellation, each
  behind a deadline so that a regression fails instead of hanging.

- **xpath:** Return paths in document order, count positions per step ([8f1f632](https://github.com/imbus/robotframework-PlatynUI/commit/8f1f63292281d4a21b2333688d4960ca9be24497))

  A path returns its nodes in document order and each node once, and a
  positional predicate in a step counts per context node, as XPath 2.0
  requires. Steps keep the order by construction where they can: child
  steps merge the children of nested context nodes, descendant and
  following steps minimize their context nodes. Only steps that cannot
  keep it are sorted, by a total order that no longer fails on a model
  whose answers do not fit together. The first match of the usual
  locator shapes reads only the part of the tree before it.

  Behavior changes:

  - Query, platynui-cli query, Inspector search and Runtime.evaluate list
    matches in document order, not grouped by tree level.
  - A keyword that acts on one element, Query only_first=${True}, the
    waits, Runtime.evaluate_single and PlatynUI.core find_one use the
    first match in document order. When matches sit at different depths,
    that may be a different element than before.
  - //X[n], A/X[n], X[last()] and X[position()=n] inside a step count per
    parent and may return several elements. (//X)[n] is the n-th match
    overall, which is what //X[n] returned before.
  - Positional predicates on reverse axes count from each context node:
    //B/ancestor::*[1] is every B's parent.
  - following:: from nested context nodes no longer loses results.
  - Paths whose order cannot be proven (sibling and reverse axes from
    several context nodes, filter-expression steps, paths from arbitrary
    sequences) collect their input before the first result. The union
    idiom .//(A|B)[p] streams when p does not count positions.
  - On the mock, //control:Window[@Name=...] first finds the copy under
    its app:Application, because the mock lists applications first.

- **xpath:** Keep evaluation records out of release builds ([6225cef](https://github.com/imbus/robotframework-PlatynUI/commit/6225cef95182d84f0142a5741a0144974c41f0df))

  Evaluating an XPath expression acts on nothing in the UI, so the
  records it writes about itself only serve PlatynUI's development.
  They are now compiled into debug builds only: the runtime adapter's
  debug records per evaluation and trace records per wrapped element,
  and the engine's trace record per materialized sequence, together
  with the size hint computed only for it.

  - Release builds, the published wheels included, no longer log XPath
    evaluation at debug or trace.
  - In debug builds the records keep their levels and fields, and
    their messages follow the style of dev-docs/logging.md §11.
  - fn:trace() is unchanged and is recorded in every build.
  - Two runtime unit tests pin the behavior in both builds: only debug
    builds carry the evaluation's own records, and a failing
    expression is returned, never logged as a warning or an error.

- **xpath:** Classify context reads inside arguments and operands ([8871d04](https://github.com/imbus/robotframework-PlatynUI/commit/8871d04a3aacf52ecc9b1bbd4b5f9f472043a141))

  is_context_dependent judged only an expression's top-level node
  selection, so every function call counted as independent of the
  context: count(.//x) did, and so did name() and position(), although
  3b4b5def meant the zero-argument context functions to count.

  It now looks into every operand, condition, binding and function
  argument, and knows the standard functions that read the context when
  an argument is left out: the registered forms that call
  require_context_item, and position() and last(). Predicates and the
  later steps of a path keep their own focus and are not looked into.

  Set Root and the element lookup use the same answer, so a selector
  that reads its context only in such a place, such as root()/x or
  let $a := .//x return $a, now counts as relative.

- **xpath:** Enforce document-node root for absolute paths (XPDY0050) ([9ad58a3](https://github.com/imbus/robotframework-PlatynUI/commit/9ad58a328717339639ccc30fa054725268f9f7f2))

  XPath 2.0 §3.2 defines a leading `/` as an abbreviation for
  `(fn:root(self::node()) treat as document-node())`. The `treat as` was
  missing: `ToRoot` walked the parent chain and returned whatever it
  landed on. With a free-standing element as context node, `/` yielded
  that element and `/root` silently yielded the empty sequence instead of
  raising a dynamic error.

  `ToRoot` now checks for NodeKind::Document and raises XPDY0050
  otherwise. It is emitted only by lower_path_expr for PathStart::Root and
  RootDescendant, so this affects exactly the `/` and `//` abbreviations;
  `fn:root()` has its own implementation and keeps working on trees
  without a document node.

  XPDY0050 did not exist in the crate before and is added to ErrorCode.

  Real UI trees cannot trigger this: RuntimeXdmNode::Element::parent()
  synthesizes a document node when the underlying UiNode has no live
  parent, so every chain terminates in one.

- **xpath:** Accept primary expression as first step of absolute path ([fa06f98](https://github.com/imbus/robotframework-PlatynUI/commit/fa06f98dbc24957d1bf2fbe8b93456e7ab70089d))

  Evaluating `/.` failed with `XPST0003: relative path starting with
  expression must be handled via build_path_expr_from_relative` — an
  internal parser invariant leaking as a user-facing error.

  The grammar was already correct: XPath 2.0 productions [27] and [38]
  make `.` a ContextItemExpr -> PrimaryExpr -> FilterExpr -> StepExpr, and
  xpath2.pest transcribes them faithfully. Only the AST builder disagreed:
  build_relative_steps rejected a leading filter_expr, assuming it always
  introduces the base of a relative path. After `/` or `//` the base is
  already the root node, so a leading primary expression is an ordinary
  step.

  Affects the whole class of such paths: `/.`, `/(a|b)`, `//.`, `/./root`.
  The remaining generic messages no longer name internal functions.

- **xpath:** Consolidate EBV into canonical streaming implementation ([316d2a3](https://github.com/imbus/robotframework-PlatynUI/commit/316d2a346223b9e70c13ee3982060664888b0b2d))

  Three divergent EBV implementations (common::ebv, Vm::ebv, Vm::ebv_stream)
  are replaced by a single canonical module (engine::ebv) with two functions:
  ebv_of_atomic for singleton atomics and ebv_of_stream for streaming sequences.

  Fixes XPath 2.0 §2.4.3 compliance: sequences whose first item is a node
  now correctly return true regardless of subsequent items. Previously,
  (node, node) and (node, atomic) sequences erroneously raised FORG0006.

- **xpath:** Remove unused serde feature from rust_decimal ([90cfdbc](https://github.com/imbus/robotframework-PlatynUI/commit/90cfdbcafa2a359fa4bacbd17a9df485099a8a8e))
- **xpath:** Correctness and quality sweep for integer subtypes, sum overflow, and fn:trace ([fe50a48](https://github.com/imbus/robotframework-PlatynUI/commit/fe50a48d867a6c52f58ac6c01302c2023ce44b96))

  Bugs fixed:
  - classify() now handles all 13 XSD integer subtypes (xs:long, xs:short,
    xs:byte, xs:unsigned*, xs:nonPositiveInteger, etc.) — previously only
    xs:integer/xs:decimal/xs:float/xs:double were recognized, causing
    "non-numeric operand" errors on subtype arithmetic
  - Fix memory leak in index_of_stream: replace Box::leak() with owned
    String + as_deref()
  - Fix silent i128→i64 truncation in sum(): use try_into() and raise
    FOAR0002 on overflow
  - fn:trace() now emits label and value via tracing::debug! per XPath 2.0
    spec (previously discarded the label entirely)

- **xpath:** Positional predicates on large sequences no longer hang ([001fabf](https://github.com/imbus/robotframework-PlatynUI/commit/001fabfdf5ad87d512d7d03a3a96732b48977be4))

  Operators like `<`, `>`, and `>=` in position predicates were missing
  fast-path recognition, causing full VM evaluation for every item.
  Combined with missing early termination, expressions like
  `(1 to 999999999)[position() < 11]` took over 5 minutes instead of
  milliseconds.

  - Recognize all comparison operators for positional fast-paths
  - Stop iteration early when no further matches are possible
  - Add comprehensive streaming tests for positional predicates
  - Update planning.md to reflect implemented XPath optimizations

- **xpath:** Support for local variadic functions with default namespace ([d87a71a](https://github.com/imbus/robotframework-PlatynUI/commit/d87a71ac576175bad3a2f080aac08cea1395cdb4))
- **xpath:** Replace f64 with rust_decimal for xs:decimal precision ([4a6aa24](https://github.com/imbus/robotframework-PlatynUI/commit/4a6aa242f4632b9f556402275c6acc118c742302))

  Migrate XdmAtomicValue::Decimal and ast::Literal::Decimal from f64 to
  rust_decimal::Decimal (128-bit exact arithmetic). This fixes IEEE 754
  rounding errors where expressions like
  xs:decimal('0.1') + xs:decimal('0.2') eq xs:decimal('0.3')
  incorrectly evaluated to false.

- **xpath:** Correct handling of wildcard test matching only elements and attributes not documents ([9c29788](https://github.com/imbus/robotframework-PlatynUI/commit/9c297884f1292997bae96073a2a73bd7f99e71b0))
- **xpath:** Correct union, except and intersect operators ([b6c36ca](https://github.com/imbus/robotframework-PlatynUI/commit/b6c36ca55ca3cf22aa1ceee1f1ba57f1986873ee))
- **xpath:** Normalize dayTimeDuration equality keys ([4bc819b](https://github.com/imbus/robotframework-PlatynUI/commit/4bc819b2a85b7e557e1e99460d453d73e625ec05))
- **xpath:** Enforce context item checks for root path compilation ([94f892f](https://github.com/imbus/robotframework-PlatynUI/commit/94f892f217f43b86fdc77bc924d8292d0fdaa7f0))
- **xpath:** Enforce spec-accurate function conversions ([1be2915](https://github.com/imbus/robotframework-PlatynUI/commit/1be29158ea3d376ac7273170878bd64d389a37c3))
- **xpath:** Correct namespace wildcards and string value concatenation ([3307676](https://github.com/imbus/robotframework-PlatynUI/commit/33076769342e625eb9867d6acfca60e6f61e3bbc))
- **xpath:** Consolidate duplicated `namespace-uri` functions ([e813823](https://github.com/imbus/robotframework-PlatynUI/commit/e8138237887fa566d28081c03aecb0c2568a0bc5))
- **xpath:** Improve error handling for logical and comparison operators, and enhance lock handling in TestNode ([8230d9f](https://github.com/imbus/robotframework-PlatynUI/commit/8230d9f76fc6ca4dd891d4605ecc70d48d7b7a16))
- Satisfy the clippy lints of Rust 1.100 ([4853ef8](https://github.com/imbus/robotframework-PlatynUI/commit/4853ef8f47c3c0458b3d10e40e33df8806622593))


  Name the native module through #[pyo3(name = "_native")] instead of an
  underscore-prefixed function, and use Option::map_or_default in the
  compositor, which declares no rust-version.
- Satisfy the clippy lints of Rust 1.99 ([6968ba0](https://github.com/imbus/robotframework-PlatynUI/commit/6968ba0df0a469a89cb281183b6a8d01f978a096))


  Clippy 1.99 adds assert_is_empty: a bare assert!(x.is_empty()) only says
  that the check failed, not what the value held. The 73 such assertions,
  all in tests, now compare with an empty value through assert_eq!/assert_ne!,
  so a failure shows the contents. A plain [] serves where the element type
  is unambiguous; Vec::<T>::new() names it where it is not.

  The same release flags clone_on_copy inside pyo3's from_py_object
  expansion for PyNamespace, the one Python class that also derived Copy.
  It keeps Clone, like every other class built that way.
- Keep the Windows-only crates and Inspector lint-clean on Linux ([d0d9072](https://github.com/imbus/robotframework-PlatynUI/commit/d0d90723d1e5a0475a5c164f671912ce40b757f2))


  The Linux clippy job failed after the workspace lints were adopted: the
  two Windows crates gate all their code behind cfg(windows) but declared
  their dependencies for every target, so on Linux each one counted as
  unused. The dependencies now sit in the Windows-only table, as in
  provider-java. The Inspector's Linux-only modifier reader also named
  PlatynUI in docs without backticks, which the same job would hit next.
- Correct update version script ([f7b88a6](https://github.com/imbus/robotframework-PlatynUI/commit/f7b88a640a6d76132f2435b240e7c49a28806718))
- Add sync command to pre-bump hooks for all packages and extras ([e310385](https://github.com/imbus/robotframework-PlatynUI/commit/e310385a823e38851d922590b212e1363cbe5034))
- Add type hints and mypy configuration for improved type checking ([b31f326](https://github.com/imbus/robotframework-PlatynUI/commit/b31f32631908a82d79a6b5ed53a25b0769940a61))


### Documentation

- **acceptance:** Describe the lane recipes as they run with the wrapper ([dfe03fd](https://github.com/imbus/robotframework-PlatynUI/commit/dfe03fd37447b8d59aab7ba37dd0e4adf2620d72))

  CONTRIBUTING, the justfile's acceptance header and a robot.toml comment
  still described the lanes from before the profile `wrapper`: the session
  script picked the profile, and extra arguments replaced the whole robotcode
  command. Neither is true on Linux any more -- the recipe runs its lane
  profile, the profile's wrapper brings the session up, and extra arguments
  are appended to `robotcode --profile … run`. Only the Windows recipe still
  replaces the default command.

  The CONTRIBUTING example repeated `--profile real-wayland run` and relied on
  shell quoting that `just` does not forward; it now uses a working suite
  filter. In robot.toml the mock-lane comment had drifted above the lane
  profiles and moves back to `[profiles.mock]`. Profile selection is
  unchanged (real-x11 81, real-wayland 81, mock 122 tests).

- **api:** Implement evaluate API variant for flexible context handling ([93ebb28](https://github.com/imbus/robotframework-PlatynUI/commit/93ebb2885c3b3c06793340258323166897b67f0d))
- **architecture:** Move provider specifics to section 10 ([fc03226](https://github.com/imbus/robotframework-PlatynUI/commit/fc032262040f06ba224cd99944f521c38abbb9e4))

  Sections 5.5 and 5.6 now state the rules for Id and Description without
  naming a technology, and the platform-specific checklists of section 7.3
  become implementation notes of each provider in section 10. Two stale
  items are corrected on the way: the window manager has no
  ensure_window_accessible yet, and native attributes are named
  native:<Interface>.<Property>.

  Section 6.2 allows a provider to ask the platform to make an element
  readable, as UI Automation's Realize does, and the catalog gains the
  runtime pattern BringIntoViewable, which brings an element into view the
  way the application does it and carries IsInView.

- **architecture:** Settle the open questions of the pattern system ([64036e3](https://github.com/imbus/robotframework-PlatynUI/commit/64036e35d737d2dfa891518dbadcb6773bc56e83))

  Desktop and Application stay patterns: Desktop requires Bounds and
  OsName, and Application, which marks an application's root whatever its
  role, is the one pattern without a required attribute, because a process
  cannot always be identified. Tables get no pattern of their own; the
  platform's structure stays as it is and its extra facts stay native.
  Clearable, Highlightable and ItemContainer are the client's own
  patterns, never announced by a provider. Bounds stays required, and a
  width or height of -1 marks it invalid. An attribute a node offers stays
  offered for as long as the node exists. WindowState is announced, and
  IsFocused belongs to Focusable. Editable gains the optional text
  attributes IsPassword and IsMultiLine.

  The remaining ideas are kept as candidates.

- **architecture:** Restate the pattern system as a contract ([71937e0](https://github.com/imbus/robotframework-PlatynUI/commit/71937e0172f2052cbadc662d5f9420bc14af62c6))

  Section 6 now describes patterns without naming a technology: what a
  pattern is, that providers only read while the client acts like a user,
  runtime versus client patterns, the rules for announcing patterns in
  SupportedPatterns, and a catalog with required and optional attributes.
  Read-only and disabled are separate states, Editable replaces
  TextEditable, SelectedItems leaves the contract, and Expandable is
  announced by the kind of element.

  How each provider fulfils the patterns moved to section 10 as a
  per-provider snapshot, together with a role table that shows roles per
  technology and toolkit. Section 5.7 states that roles are not part of
  the cross-platform contract.

  The draft keeps its remaining open questions in section 6.7. References
  to the old section numbers point to the new ones.

- **architecture:** Rewrite as concept-first prose, not API reference ([334703a](https://github.com/imbus/robotframework-PlatynUI/commit/334703aeae747e6656a3422c760dcefa76e30fa0))

  Rewrite all 11 sections to read as architecture (concepts,
  responsibilities, the "why") rather than as an API reference; the
  maintainer found the prior signature-dense version too technical.

  - Replace pasted Rust trait/struct/signature blocks (UiNode, the nine
    window-capability patterns, WindowManager, XdmNode, the mock-link
    idiom) with prose, each pointing to the code as the source of truth
    for exact signatures. This also removes a standing source of doc/code
    drift.
  - Preserve every data table (namespaces, RuntimeId, event capabilities,
    the pattern catalog, the platform-mapping tables, the Wayland protocol
    table), the ASCII diagrams (crate tree, dependency graph, layer
    model), the "intended capability model" note, and all cross-references.
  - Keep the naming rule accurate: the platynui- prefix is set in each
    crate's Cargo.toml [package].name, so the directory omits it.

  Facts unchanged; presentation only. 758 -> 715 lines.

- **architecture:** Fix code drift; frame §6 catalog as intended model ([bfa4f5d](https://github.com/imbus/robotframework-PlatynUI/commit/bfa4f5d0636938c68ae1cfde6c15126136b7075f))

  A section-by-section audit against the current code surfaced drift that
  accumulated since the last doc sync. Corrections (code is the source of truth):

  - §9.3 XdmCache: Rc<RefCell<...>>/!Send -> Arc<Mutex<...>>, Clone+Send+Sync
  - §4 init: initialize_platform_modules() -> PlatformModulesLease::acquire()
    (reference-counted across live runtimes)
  - §5.6 UiValue: Float -> Number(f64)
  - §5.1 PatternRegistry: HashMap<PatternName, RegistryEntry> + insertion-order Vec
  - §7.1 UiTreeProvider: create() is fallible; add subscribe_events()/shutdown();
    correct get_nodes() signature
  - §7.2 RuntimeEventListener: keeps no snapshot/dirty state; invalidate() runs
    only for NodeUpdated
  - §8.1 PointerProfile: only named_default() exists (no fast/human-like presets)
  - §8.3 Screenshot: raw buffer field is `pixels`, plus a `format`
  - §2 crate tree: add platform-linux-wayland
  - §3: list all eight register_* macros (was three)
  - §8 WindowManager: the Wayland backend is implemented and wired, not "future"
  - §4 shutdown: note the event-dispatcher step
  - §9.4: document is_context_dependent()
  - §6.3: fix dangling planning.md §8.5 reference

  The §6 pattern catalog is intentionally a design/idea space (some patterns
  implemented, some planned, some may still change shape). Added a framing note
  pointing to pattern_names in crates/core/src/ui/identifiers.rs as the
  authoritative implemented set, and added the existing Clearable pattern.

- **baremetal:** Document how waits treat errors and the root ([7d3aadc](https://github.com/imbus/robotframework-PlatynUI/commit/7d3aadcaf1ee46d29b9727419c13f9f203387081))

  - The text on ignore_exceptions says that a timeout quotes the last
    error and which errors end a wait anyway, and explains the setting
    with an XPath example instead of provider errors, which cannot reach
    a wait.
  - Scope says that a root keeps the element it found while that element
    exists and that a waiting keyword checks it on every attempt.
  - The wait keywords name their failures, validate examples cope with
    every value, and the examples compare numbers with ${0}.
  - Query is no longer recommended for checking that a dialog has gone,
    Get Attribute Value says that it waits for its element, and element
    arguments get a type documentation written for users.

- **baremetal:** Expand the library guide and anchor selector examples ([6457d63](https://github.com/imbus/robotframework-PlatynUI/commit/6457d63a44e61259856d20773b7acf30f9a5cce2))

  Rework and expand the BareMetal library (Libdoc) documentation. No
  behaviour change.

  - Selector examples now model the recommended practice: a top-level
    window is reached with a plain `Window` step (never `//Window`), and
    elements are anchored under their window/group/application instead of
    searched desktop-wide. A new "Narrowing the search" section teaches it
    as a progression: broad -> window -> group -> application/ProcessId.
  - Expand "Waiting for elements": why keywords wait, what each query
    setting (timeout, retry_interval, ignore_exceptions) is for and when to
    change it, and how the import / scope / per-call levels combine.
  - Add a "Window control" section explaining capability-based window ops.
  - Fix a namespace claim: an application's ProcessId is a bare control:
    attribute (@ProcessId), not @app:...; align the Namespaces section with
    the Targeting example and the engine.
  - Turn in-text section references into real Libdoc links (single
    backticks) instead of bold, and add cross-references.

  Also annotates `old` in set_query_settings as `QuerySettings | None`
  (no functional change).

- **baremetal:** Rewrite library intro and fix keyword docstrings ([d4818e2](https://github.com/imbus/robotframework-PlatynUI/commit/d4818e2abb08e85fdc14892c951774abbd04c6c7))

  Restructure the library documentation around Query as the central concept: Querying the UI tree (with "What a query returns": XPath sequence -> list semantics, only_first, and the element/attribute/value result types), Searching the tree, Scoping a query to a parent (root=), Namespaces, Targeting a specific application, Navigating with axes, Reading values, Common attributes, then Acting on elements, Set Root (with RF-variable scoping), Waiting, Assertions and a short example.

  Also fix inaccurate keyword docstrings: Take Screenshot examples (file_path -> filename), the bogus pointer ValueError clauses, the Highlight descriptor/rect precedence, the _resolve_screen_point return contract, the count() example (only_first), exact/case-sensitive matching, the @assertable parameters, and document the __init__ import arguments.

- **cli:** Expand command help to match the CLI reference ([b65362c](https://github.com/imbus/robotframework-PlatynUI/commit/b65362c637977391be2258d82b5c350a15d87136))

  Enrich understated clap about-texts (e.g. snapshot exports text or XML) so the generated CLI help matches docs/cli.md. Help text only; no behavior change.

- **config:** Correct the claim that pyright runs in CI ([58ec4d5](https://github.com/imbus/robotframework-PlatynUI/commit/58ec4d5de184f752a0a481835907b50b5683d788))

  The `type-abstract` rationale stated that pyright is "also part of CI".
  It is not: pyright is a dev dependency and runs in the editor and on
  demand, but neither `just check` nor .github/workflows/ci.yml invoke it,
  so mypy is the only automated type gate.

- **contributing:** The Windows acceptance lane needs an unlocked session ([fe7ddec](https://github.com/imbus/robotframework-PlatynUI/commit/fe7ddec59fe71175d3df090b72a6f966e610bf3d))

  Windows is the one lane without an isolated session — it drives the real
  desktop and synthesizes real input for several minutes — so a lock
  screen, a screensaver requiring sign-in, or a dropped RDP connection
  takes that desktop away mid-run.

  Worth writing down because the failure impersonates a permissions bug:
  `SetCursorPos` returns `E_ACCESSDENIED`, the keyboard provider reports
  "not ready" and `BitBlt` fails, when what actually happened is that
  nobody is at the desktop any more. Both observed shapes are described,
  including the misleading one where an idle timer fires mid-run and only
  the pointer-dependent suites after it fail — which reads like a defect
  localised to one suite.

  Documented rather than diagnosed in the product: the providers' errors
  are accurate about what the API returned, and this is a property of how
  the lane is run.

- **contributing:** Document the egui acceptance lane and how to run it ([643bcc2](https://github.com/imbus/robotframework-PlatynUI/commit/643bcc2d68bc91733bc1684e4174602fe8913071))

  Replace the stale "when Robot suites are added" placeholder in the testing
  section with a real description of the egui acceptance lane: what it exercises
  (the real platform provider against platynui-test-app-egui), the non-mock build
  requirement and how the test-acceptance recipes guarantee it, a per-OS table of
  run commands, the headless/CI mode (with the GPU/software-GL caveat), and the
  platform-sensitivity note. Add a discoverable `just test-acceptance` row to the
  dev-workflow recipe table.

- **contributing:** Consolidate development workflow ([b538e87](https://github.com/imbus/robotframework-PlatynUI/commit/b538e87cd730e72570b177edcedda3987dc6cd55))
- **copilot:** Correct terminology for packaging and update repository layout details ([886a7b2](https://github.com/imbus/robotframework-PlatynUI/commit/886a7b27eb45ad39bdcf8ac5ac76bc7e40708b0e))
- **core:** Rewrite docstrings in PEP 257 stdlib style ([936ec82](https://github.com/imbus/robotframework-PlatynUI/commit/936ec826bcc6c8152e6f696e0e26007d143bb046))

  Rewrite all docstrings in the Python core modules to follow PEP 257
  with concise, imperative descriptions modeled on the standard library.
  Strip Sphinx cross-reference roles (:meth:, :class:, :func:, :attr:,
  etc.) in favour of plain backticks, add docstrings to AdapterLike
  Protocol members and Element abstract properties, and remove a
  duplicated function body in ensure.py.

- **core:** Align attribute names and architecture ([cb5579a](https://github.com/imbus/robotframework-PlatynUI/commit/cb5579a2583202687bc84407e1d04c2c46ec8632))
- **design:** Refine ElementDescriptor and BareMetal coexistence (Rev. 24) ([c42826e](https://github.com/imbus/robotframework-PlatynUI/commit/c42826ebb57ec91b5e4142b5c3cee1ea464acde5))

  §A.7 ElementDescriptor: drop the misleading 'shared with BareMetal'
  claim and make the actual scope explicit. core/descriptor.py is the
  high-level variant for the PlatynUI library and wraps Locator/
  ContextBase, not UiNode. The swappable storage hook exists so that
  both library inits can later install the same EXECUTION_CONTEXTS
  override against ${PLATYNUI_ROOT_ELEMENT} without core/ depending
  on Robot. PatternT is a phantom TypeVar; __call__ is keyword-only.

  §13.1 BareMetal vs. PlatynUI: rewritten as permanent coexistence
  (was 'BareMetal can later collapse into a diagnostic role', which
  contradicts the actual plan). Adds an explicit 'Shared State' block
  covering the Runtime singleton, the shared root variable, and
  value interop between both libraries.

  §13.2 UiNodeDescriptor migration: expanded from a 4-line stub to a
  proper transition plan. Documents the current state (own descriptor
  with its own root variable), the target (BareMetal variant of the
  descriptor that resolves to UiNode), and a 4-step ordering. Steps
  3+4 are explicitly out of scope for Phase 3.

  Rev. 24 banner updated to reference §13.1/§13.2.

- **design:** Add §A.4b AdapterFactory layer (Rev. 23) ([c29a93d](https://github.com/imbus/robotframework-PlatynUI/commit/c29a93d545ee6ad7ffc841df3524e6aed872ca54))

  Introduce a dedicated AdapterFactory ABC plus default
  RuntimeAdapterFactory in the design doc, separating locator-driven
  search from the passive Adapter abstraction and the native Runtime
  wrapper. Adds native_node mention in §A.4a, rewrites the §4.4
  resolution chain, updates §1.2/§6.4/§9, and reorders the migration
  phases.

- **design:** Add Python library design with AccessKit smoke findings ([ceb3057](https://github.com/imbus/robotframework-PlatynUI/commit/ceb3057123ffa76a711a82b3a44a7baf5d7bcdc1))

  Introduces docs/python-library-design.md as the migration blueprint for
  porting the legacy Python PlatynUI library onto the Rust core. Document
  covers patterns-in-Python architecture, AdapterProxy override mechanism
  via @pattern_proxy_for, semantic Robot keywords with outcome contracts,
  testing strategy (3-tier pyramid), and real-provider test harnesses.

  Rev. 13 captures the Phase 0 smoke verification on Linux/Wayland/GNOME:
  accesskit_unix only registers its AT-SPI adapter when
  org.a11y.Status.ScreenReaderEnabled is true (see
  accesskit_unix-0.21.0/src/context.rs:153-180), making egui/winit apps
  invisible to AT-SPI without an explicit toggle. The verified workaround
  is documented in §11a.7.1, with side-findings (AT-SPI timeout, missing
  ToolkitName) in §11a.7.

  Adds scripts/linux-a11y-enable.sh and scripts/linux-a11y-restore.sh
  (referenced from §11a.7.6) that toggle ScreenReaderEnabled idempotently
  via gdbus, persisting the prior value in $XDG_RUNTIME_DIR for
  restoration. Both scripts are no-ops on non-Linux platforms so RF Suite
  Setup/Teardown can call them unconditionally. Roundtrip verified
  locally (false→enable→true→restore→false, plus idempotency).

- **keyboard:** Refine runtime contract ([e6a3d9d](https://github.com/imbus/robotframework-PlatynUI/commit/e6a3d9dd3cf1f5af4d7c91a59734d4380cf05c43))
- **logging:** Compile XPath evaluation records into debug builds only ([8a72d26](https://github.com/imbus/robotframework-PlatynUI/commit/8a72d26d9f2bbda237ca406fa547f178f5a8058e))

  dev-docs/logging.md §12 gets a fourth producer rule: XPath
  evaluation's own records exist in debug builds only. It explains why,
  what the rule covers, how a record is gated with
  #[cfg(debug_assertions)] together with its helper variables and
  imports, why tracing's level features cannot do it, and which builds
  carry the records. fn:trace() is the exception, and XPath failures
  are returned, never logged.

  - The checklist states the rule in one line in §5.
  - The debug examples in §3 and in the checklist no longer use an
    XPath record, which release builds no longer contain.
  - §19 maps the new diagnostic-logging requirement to §12, and the
    spec's Purpose names it.

- **logging:** Write the logging concept and its checklist ([0688de1](https://github.com/imbus/robotframework-PlatynUI/commit/0688de1f043c87b65c6b738b098965b4cd5ec4df))

  dev-docs/logging.md is the normative logging concept for Rust and
  Python: one meaning per level and who sees it, log or return, once per
  episode, decisions that name their outcome, the message style and field
  names, the element description, typed text and Secret, the Python
  loggers, the level knob, and how to look at a run's warnings.

  .github/instructions/logging.instructions.md replaces the Rust-only
  tracing instructions with a checklist for Rust and Python that links the
  concept. The documents that contradicted it are corrected: error
  handling, the keyboard escape rules, the logging sections of the Python
  bindings and of the Python library design, and the compositor plan.
  AGENTS.md lists the concept and the new crate.

- **openspec:** Propose fix-shared-runtime-id-hang ([f2f8d4f](https://github.com/imbus/robotframework-PlatynUI/commit/f2f8d4f15078e93757acb1ae4f3d26921283e5b3))

  UI Automation reports an empty runtime id for MSAA-proxied elements
  without a window of their own, such as the tab headers of a WinForms
  TabControl on .NET Framework. The provider gives all of them the same
  id, and the XPath engine finds a node's next sibling by identity, so
  any query that walks past such siblings never ends. Found against
  KeePass 2.62 on the Windows VM; main and 0.13.0.dev301 behave alike.

  The change derives ids from the parent plus the MSAA child id or the
  position, moves the engine between siblings by position, checks the
  cancel flag for every visited node, and adds a WinForms helper window
  for the Windows acceptance lane.

- **openspec:** Add the publish-workflow change ([de4ec4b](https://github.com/imbus/robotframework-PlatynUI/commit/de4ec4ba528413d5d6d1180528656598b9445e31))

  Proposal, release-publishing spec, design and tasks for publish.yml:
  nightly dev builds of main and releases of version tags on PyPI
  through Trusted Publishing, a -dev.0 tag that starts a version line,
  and a changelog step that stops cz bump when git-cliff is missing.

  Groups 1 to 5 and the local checks are done. Pushing, the start bump
  to 0.14.0-dev.0 and the first manual run of publish are still open.

- **openspec:** Archive add-gtk-test-app ([02c01c1](https://github.com/imbus/robotframework-PlatynUI/commit/02c01c19358b98a64dd5fef9509df3c902d260d2))

  Sync its delta specs into the main specs: the new gtk-test-app
  capability, and platform:linux in acceptance-lane-selection on top of
  the bare X11 lane linux-element-geometry added.

- **openspec:** Complete add-gtk-test-app ([9104c88](https://github.com/imbus/robotframework-PlatynUI/commit/9104c8800cf793bad8f908d566cd95429ed73d5b))

  CI run 44391763278 passed every job: the Linux jobs that bootstrap the
  environment, the wheel builds, and all three acceptance lanes, which
  loaded GTK 4.22.4 and passed the GTK suites with the popover tests.

- **openspec:** Record GTK 4's stray menu and merge the bare X11 lane ([aca3bbf](https://github.com/imbus/robotframework-PlatynUI/commit/aca3bbfce6bfa4f4b0671a56c90f57f5c41bf28d))

  add-gtk-test-app records why the GTK 4 submenu test runs on an instance
  of its own and why GTK 3 menu tests take a screenshot on failure. Its
  acceptance-lane-selection delta now carries the real-x11-bare profile
  that linux-element-geometry added to the main spec.

- **openspec:** Verify add-gtk-test-app in every CI matrix entry ([38f4c75](https://github.com/imbus/robotframework-PlatynUI/commit/38f4c7520d6d43abe516b1ddf1685b4a26023b57))

  The acceptance job gained the x11-bare entry, which runs the GTK suites
  too.

- **openspec:** Archive linux-element-geometry ([6dfea9e](https://github.com/imbus/robotframework-PlatynUI/commit/6dfea9e4f14e5d129afb2d79e3cda10059a83181))

  Sync its delta specs into the main specs: new capabilities
  atspi-element-geometry and x11-window-management, and the modified
  requirements of acceptance-lane-selection, compositor-popup-geometry,
  element-at-point and wayland-compositor-detection.

- **openspec:** Complete linux-element-geometry ([0bb8d15](https://github.com/imbus/robotframework-PlatynUI/commit/0bb8d15e05e6b3f0a45d26a6c49f0053cb5cb397))

  All tasks are done. The design and specs record what implementing it
  found: without a window manager toolkit screen coordinates are not
  desktop coordinates, GTK 3 menus are matched by their visible size
  and hand their reference to the items listed beside them, activation
  without a window manager keeps the process's popups above the window,
  and the measured GTK 4 drift. add-gtk-test-app marks its runs after
  the provider rework as done; its CI checks wait for a push.

- **openspec:** Place GTK 4 popovers in linux-element-geometry ([48daef4](https://github.com/imbus/robotframework-PlatynUI/commit/48daef459f7cc22fa71ad2e8f49bab874c197057))

  GTK 4 reports a popover's position measured from the widget that
  opened it, without the popover's own placement. Measured on GTK
  4.24.1, a nested submenu item was drawn about 100 px from where it
  was reported, so its click missed.

  The change now places popups where the platform shows them: a node on
  the popup layer becomes the reference window of its contents, its
  position comes from the platform's popup list matched by process and
  surface size, and a hit inside a popup wins in the hit-test. The
  compositor's popup listing gains each popup's surface, and the X11
  backend lists the process's popup windows. add-gtk-test-app's GTK 4
  popover tests join the first failing tests and are committed with the
  provider rework.

- **openspec:** Apply add-gtk-test-app before linux-element-geometry ([0338998](https://github.com/imbus/robotframework-PlatynUI/commit/03389984090b46a83a4c4f695e74041bab189e77))

  Real GTK fixtures first give the geometry work its acceptance tests
  from the start. The GTK 3 fixture is built now; only its pointer
  tests wait for the provider rework and are committed with it as that
  change's first failing tests. The geometry change's separate GTK group
  dissolves into its X11 and provider groups.

- **openspec:** Propose add-gtk-test-app ([1ed1573](https://github.com/imbus/robotframework-PlatynUI/commit/1ed157350f9085800bd818ada06b3e52db773c7b))

  GTK 3 and GTK 4 blueprint fixtures in Python with PyGObject from the
  project environment, run on both Linux lanes. A platform:linux tag
  keeps them off the Windows lane, and a requires:gtk-4.18 tag selects
  the popover tests only where GTK 4 places popovers correctly.

- **openspec:** Sequence linux-element-geometry with add-gtk-test-app ([768e094](https://github.com/imbus/robotframework-PlatynUI/commit/768e094f637f6623289d1dc222aa6600ac7fd29d))

  The GTK 3 fixture needs this change's provider rework, so the two
  changes interleave instead of the GTK change coming first. The bare
  X11 profile and the window-manager tags also change lane selection,
  which the change now states as a delta to acceptance-lane-selection.

- **openspec:** Propose linux-element-geometry ([cda1217](https://github.com/imbus/robotframework-PlatynUI/commit/cda1217586647b7e9bd1c8d22f8dda01d440cff6))

  Place AT-SPI elements relative to their window instead of summing
  parent-relative extents, read each toolkit in the coordinate type it
  implements consistently, and let the platform report a window's visible
  geometry and surface origin separately. The X11 backend also finds and
  operates windows when no EWMH window manager runs.

  GTK acceptance waits for a separate add-gtk-test-app change.

- **openspec:** Align jab-discovery-containment with the main specs ([4165ce0](https://github.com/imbus/robotframework-PlatynUI/commit/4165ce01a5e1ea7c12218bd5bc8f2546096fc61a))

  Its MODIFIED blocks for java-provider and jab-hit-test carry the same
  requirement text the main specs now use, including the scenario
  renamed to "The JAB backend serves the Swing fixture". Archiving the
  change therefore neither brings back "as today" and "exactly as
  before" nor fails on a scenario the main spec lists under its new
  title.

- **openspec:** State requirements without change-relative wording ([9e5cfd4](https://github.com/imbus/robotframework-PlatynUI/commit/9e5cfd40062a26095f6abf5ed487d36dada34cc1))

  Requirement and scenario text across the main specs still measured
  behavior against the moment its change was written: "as today",
  "exactly as before this change", "as before the refactor", "Regression
  anchor", and titles ending in "is unchanged" or "is preserved". Each
  statement now names what it compares with, or says what holds without
  a comparison, and each title says what its scenario shows. The
  requirements that jab-discovery-containment modifies are reworded the
  same way in its delta, so archiving it keeps this wording.

- **openspec:** Keep change-relative notes out of specs ([d309e63](https://github.com/imbus/robotframework-PlatynUI/commit/d309e6309d50ac0e3c6ad4d1eef6fd8077210670))

  Scenario notes across the main specs still described the code as it
  was before their change: "Before this change ...", "Today ..." and
  measurements taken during the investigation. Every such claim was
  checked against the code and is outdated, so it goes. What a note says
  about how its scenario is verified stays, and so does its rationale.
  References into archived changes now name the archived change, and
  description-attribute no longer claims that no automated UIA lane
  exists. Requirement text that still says "today" is left for a
  separate pass.

- **openspec:** Keep change-relative notes out of specs ([cbd2f8d](https://github.com/imbus/robotframework-PlatynUI/commit/cbd2f8dbcbb831f5c0f0c7b859c6a353e4170e8b))

  A scenario note that says what the code does today turns false once
  its change is archived into the main spec. The specs rule now says a
  note states how a scenario is verified and leaves the behavior before
  a change to the proposal. id-attribute drops the five notes of that
  kind that earlier archives left behind.

- **openspec:** Archive name-is-accessible-name ([0e93016](https://github.com/imbus/robotframework-PlatynUI/commit/0e9301665186ae8334f59fe4d857d0b68b983634))

  The new name-attribute spec states that Name is the accessible name
  the toolkit reports and nothing else, that it is always present and
  may be empty, and where the values that are not the name stay
  readable. description-attribute names the sources of the Java Access
  Bridge and the Java agent, textcontent-pattern the agent's text
  source, and swing-test-app gains the name-source panel. id-attribute's
  table-cell scenario names a cell by the text it displays. Notes that
  described the code before the change are kept out of the main specs.
  The change moves to the archive.

- **openspec:** Propose add-docs-site ([139ad06](https://github.com/imbus/robotframework-PlatynUI/commit/139ad0694c0b0c7f7d6baca08a7f80de1fa48fb8))

  The user documentation website for Robot Framework users: Astro and Starlight in docs/, writing rules, link checking, examples as tested .robot files, and GitHub Pages deployment with an address derived at build time.

- **openspec:** Record the Linux lanes of name-is-accessible-name ([bc5abc4](https://github.com/imbus/robotframework-PlatynUI/commit/bc5abc4ad3a0484a9da4e68396f755484a30d3e8))

  The X11 and compositor lanes pass in full, 102 and 103 tests, with no
  WARN or ERROR. The egui query suite keeps its Name, Id and Description
  checks green, and no Qt, QML or egui suite relied on the AT-SPI name
  fallback this change removed. Every task of the change is now done.

- **openspec:** Propose add-demo-app ([4d7211d](https://github.com/imbus/robotframework-PlatynUI/commit/4d7211dc5b102c821e654c3a2fa40b56759f333f))

  A QML café point-of-sale app, shipped as platynui-demo, as the target the user documentation teaches with. Starts with a spike on how Qt Quick's accessibility reaches UIA and AT-SPI.

- **openspec:** Record the name-is-accessible-name implementation ([9f57ea1](https://github.com/imbus/robotframework-PlatynUI/commit/9f57ea157a9968855ab49e08ac1089584f25c74e))

  The tasks record what is done, from the fixture and the tests that
  failed first to the Windows lane, and the design records the full-walk
  timing of the agent's table before and after the change. The X11 and
  compositor lanes (6.3) are still to run.

- **openspec:** Name tree nodes by their accessible name ([7a79cdc](https://github.com/imbus/robotframework-PlatynUI/commit/7a79cdcc53059797deee6aa5fed08089071d8efb))

  java-agent-tree-items took a tree node's name from the model, which
  name-attribute now rules out. Its design, spec and tasks take the name
  from the node's accessible wrapper like every element's, and its text
  only where Swing provides one, while structure, expansion and
  selection still come from the model.

- **openspec:** Archive xpath-document-order ([f291770](https://github.com/imbus/robotframework-PlatynUI/commit/f291770f145360d3d46780c43cb99dfbae4e2abd))

  xpath-evaluation and core-locator become main specs: a path returns
  its nodes in document order, positional predicates count per context
  node, and a locator's index and position count over its whole scope.
  baremetal-selector-resolution gains the rule that several matches
  resolve to the first in document order, and java-provider the runtime
  ids per view. The change moves to the archive.

- **openspec:** Record the CI run of xpath-document-order ([7c8fcc5](https://github.com/imbus/robotframework-PlatynUI/commit/7c8fcc50e7eaa97987da289a50e1d3c5f328c171))

  CI run 36831829876 on main passed every job; the X11 and compositor
  lanes passed 102 and 103 tests without a WARN or ERROR from PlatynUI.
  This completes the tasks of the change.

- **openspec:** Keep agent Text to what Swing provides ([812c8e6](https://github.com/imbus/robotframework-PlatynUI/commit/812c8e61043d93052e431b913e81a672ade48673))

  Swing's labels and buttons, and the renderers of cells, column headers
  and list entries, return no AccessibleText unless their text is HTML,
  on JDK 8 and 21 alike, so the planned Text for them had no source. The
  agent reports only the text Swing provides and invents none: plain-text
  elements keep no Text, as through the bridge, and HTML text gains it.

- **openspec:** Archive attribute-values-as-reported ([da32ef9](https://github.com/imbus/robotframework-PlatynUI/commit/da32ef95e4c6635f45d29f1f3192bd64afe1c158))

  The new attribute-values spec states that every provider passes values
  through as the platform reports them, and when the empty string is a
  value in each attribute group. id-attribute and description-attribute
  take the identifier and the description unmodified, with the AT-SPI
  whitespace scenarios. The change moves to the archive.

- **openspec:** Record the Linux run of attribute-values-as-reported ([6f1ff8d](https://github.com/imbus/robotframework-PlatynUI/commit/6f1ff8dca0276a2c45d5c953a4db836a65e13449))

  The X11 and compositor lanes passed without WARN or ERROR, so the last
  task is done and the change is ready to archive.

- **openspec:** Record the xpath-document-order implementation ([7fc00e2](https://github.com/imbus/robotframework-PlatynUI/commit/7fc00e2dfbb22ac722a8786913f04a332f63e274))

  Align design decisions 4 to 6 with the code: every step keeps its
  output in document order and the compiler tracks only whether a stream
  is at most one node, every child step runs as the ordered merge, and a
  comparison reads a list of children only as far as it needs. Add the
  scenario for a container taken with (...)[1], record the Windows
  verification and measurements with the suite rewrite of 12.2, and let
  runs in the Windows VM go without asking.

- **openspec:** Record the Windows baseline of xpath-document-order ([52a4df5](https://github.com/imbus/robotframework-PlatynUI/commit/52a4df5ceaa86c24cbc4fd2a386c7697d5ecbaa2))

  Task 1.1 measured the positional table selectors and a full walk of the
  Swing fixture on 75a9bd3b, through the agent and the Access Bridge. All
  18 tests passed. The slowest call is .//*[@Name="main-table"]/*[544] on
  the bridge: 1,006 ms the first time and 134 ms after that. Task 12.2
  measures again with the suites and the script kept in
  results/xpath-document-order-baseline/.

- **openspec:** Plan the .//(A|B)[p] rewrite in xpath-document-order ([6e452dd](https://github.com/imbus/robotframework-PlatynUI/commit/6e452dd11c831b30db16b2cc57a2ce57e5a8d98b))

  The union idiom .//(A|B)[p] was an open question of the design. It now
  becomes one descendant step when no predicate counts positions, and the
  compiler rewrites it while it lowers the path, because the optimizer
  sees the union only as an opaque path-expression step. Today //(P|B)
  already comes out as [P1, B3, B1, B2, P2, B4, B5]; new scenarios pin
  the order and the per-parent count of //(P|B)[1].

  The plan test of task 3.6 is split in two, so that the rows of group 6
  compile before group 7 adds the normalization op. Only the Windows lane
  needs the maintainer's go-ahead, and 12.1 runs the Linux lanes locally.

- **openspec:** Align xpath-document-order with the current code ([5079c93](https://github.com/imbus/robotframework-PlatynUI/commit/5079c93057e9703fa3a469770453e01e93b1ce74))

  The design's line references were checked on 999ce95. They now point
  at 65cd1c17; the engine has not changed since, so its measurements
  still hold. xdm-snapshot-release, application-process-attributes and
  snapshot-validity are archived: the coordination and the migration
  plan say so, task 1.2 is gone, and the open changes that edit other
  functions of the agent's files are named.

  The predicate classifier shares its focus walk with
  is_context_dependent. Suite filters go without spaces, because just
  passes its arguments on unquoted, and the German design doc already
  has its English summary.

- **openspec:** Archive xpath-debug-build-records ([882c66e](https://github.com/imbus/robotframework-PlatynUI/commit/882c66ef37073a445e50736affec69dc99e6b87b))

  diagnostic-logging gains the rule that XPath evaluation's own records
  exist in debug builds only, that fn:trace() is recorded in every build,
  and that XPath failures are returned, never logged. Its level list
  names a keyword action line instead of an XPath evaluation as an
  example of a debug record. The change moves to the archive.

- **openspec:** Record the xpath-debug-build-records implementation ([ec4cb4b](https://github.com/imbus/robotframework-PlatynUI/commit/ec4cb4ba956bfa4f061d3143fa202f9fa2950b44))

  The tasks record the implementation and its verification in debug
  and release builds. Proposal and design take in the maintainer's
  answers of 2026-09-30: the ADDED requirement stays, there is no
  standing release check for now, and the records' messages follow
  dev-docs/logging.md §11 (design decision 7, task 2.3).

- **openspec:** Record the attribute-values-as-reported implementation ([9de4fbe](https://github.com/imbus/robotframework-PlatynUI/commit/9de4fbe314443417afbfd74f1dc46846e2fa7738))

  The tasks record what is done: the tests that failed first, the
  provider and agent changes, the docs, pre-commit and the Windows lane.
  The X11 and compositor lanes (4.3) are still to run.

- **openspec:** Propose naming elements by their accessible name ([6c2946c](https://github.com/imbus/robotframework-PlatynUI/commit/6c2946c28800f82c3aea0794483667d0d7048507))

  The Java agent names components by their developer id, windows by
  their title and table cells by their model value, and AT-SPI falls
  back to object attributes, so @Name means something different per
  provider and the same Swing application answers differently through
  the bridge and the agent. The change pins Name to the accessible name
  and keeps the displaced values readable elsewhere.

- **openspec:** Propose passing attribute values through as reported ([cc0da8f](https://github.com/imbus/robotframework-PlatynUI/commit/cc0da8f86af0cbbbc4be39a2cd5b51897de9f627))

  Providers disagree on what an attribute value is: AT-SPI trims every
  string it reads, UIA drops empty native strings, JAB trims value
  strings, and the Java agent never sends empty ones. The same element
  therefore answers differently depending on the provider. The change
  pins one rule for all of them and lands before name-is-accessible-name,
  which builds on it.

- **openspec:** Archive uniform-wait-keywords ([d73d1a0](https://github.com/imbus/robotframework-PlatynUI/commit/d73d1a04fdaaeb692e030a0f3bffdff1e71222be))

  baremetal-waiting gains the rule that every wait polls and fails the
  same way, and its requirements for Wait Until Gone, its still-present
  error, Wait Until Query and Wait Until Attribute Value follow it.
  baremetal-selector-resolution says that every keyword, Query included,
  looks the root up only for a relative selector or expression, and that
  a root is looked up on every attempt of a wait. The change moves to the
  archive, with its last task checked off.

- **openspec:** Record the uniform-wait-keywords implementation ([075106d](https://github.com/imbus/robotframework-PlatynUI/commit/075106d21c1c0b21f05c21d62ecca222e73e12b8))

  The tasks, proposal, design and spec deltas record the implementation
  and its review: validate's RuntimeError, the deeper context
  classifier, the clock start, Query, and the reworked documentation.
  The baremetal-waiting Purpose names the shared polling rule, and the
  three absorbed triage entries record that they are implemented.

- **openspec:** Record the check of the logging follow-ups ([706cb12](https://github.com/imbus/robotframework-PlatynUI/commit/706cb122c7c6bb37bb75eda264c4a002e09756ac))

  A second check held every open entry of the logging review against the
  code and the open changes. No entry was fixed in the meantime, but
  several moved. The A follow-ups and the XPath decision are open changes
  now. A2 is re-cut: the causes that waits lose under ignore_exceptions go
  to uniform-wait-keywords, and the pointer's double report stays a small
  fix. Two dropped discovery entries are reopened, because their premise
  was false. Other entries now name the open change that takes them over
  or that they must follow. Each move is a note marked "Revised
  2026-09-29" at the end of the entry's Status line, and a paragraph under
  the preamble sums them up.

- **openspec:** Propose reporting the Inspector's start failure ([89e331f](https://github.com/imbus/robotframework-PlatynUI/commit/89e331fe27e28c23720771a414c0a35927b2268e))

  A release build of the Inspector on Windows has no console, so a failed
  start leaves no trace, and a runtime that cannot be created panics. The
  change reports every failure that ends the Inspector once, on the
  terminal it was started from or in a dialog, and returns the runtime
  failure. It is follow-up A5 of the logging triage.

- **openspec:** Propose Windows keyboard diagnostics ([692a354](https://github.com/imbus/robotframework-PlatynUI/commit/692a354b688ea9520b645b9b0b1c6a3cfd46c3d2))

  On Windows a refused SendInput fails as "not ready" without the OS
  error, input to a process of higher integrity is dropped without a word,
  and a character outside the BMP is typed as a different one. The change
  returns the Win32 error, warns once per process of higher integrity, and
  sends surrogate pairs. It is follow-up A3 of the logging triage.

- **openspec:** Propose Java agent attach diagnostics ([3c73e4d](https://github.com/imbus/robotframework-PlatynUI/commit/3c73e4db082d751e3271ca75122dbd3b9de43c14))

  A mistyped agent JAR path, a version mismatch or a broken provider-java
  install switches automatic attachment off with a debug record at most,
  and the in-process interpreter the java-agent spec describes was never
  wired. The change names each cause with its remedy, warns once for a
  mistake or a refused attach, and hands sys.executable to discovery at
  import. It is follow-up A4 of the logging triage.

- **openspec:** Propose quiet X11 and AT-SPI healthy runs ([d7df049](https://github.com/imbus/robotframework-PlatynUI/commit/d7df0491435dc3e533ae7b8f84836969a743e74b))

  Healthy X11 and AT-SPI runs warn: about EWMH support twice per runtime,
  about a missing _NET_WM_PID that is no window manager hint, about a
  window lookup on every read, and about every slow get_children call. The
  change reports window manager support once per display, moves returned
  lookup failures to debug, and warns once per episode for an application
  that stays slow. It is follow-up A1 of the logging triage.

- **openspec:** Propose one polling loop for all waits ([cf7f6ae](https://github.com/imbus/robotframework-PlatynUI/commit/cf7f6ae749c0ccb7b6f12716ac5d1c24e339e6a6))

  Element lookup and the three Wait Until keywords poll in four different
  ways. Two of them swallow a vanished Set Root root under
  ignore_exceptions, and the timeout messages hide what the last attempt
  saw. One shared loop looks the root up on every attempt without
  swallowing its error, ends at once on usage errors, and reports the last
  result or "The last error was: ...". It modifies baremetal-waiting and
  takes over the logging-triage entries about lost causes.

- **openspec:** Propose debug-build-only XPath records ([e72a9e9](https://github.com/imbus/robotframework-PlatynUI/commit/e72a9e9e120ce5f5e71ebfd434f38d8485f84ab3))

  XPath evaluation does nothing on the UI, so its internal diagnostic
  records exist in debug builds only, and release builds carry none.
  fn:trace() stays in every build, and XPath failures stay returned
  errors. The change gates the five records with cfg(debug_assertions),
  states the rule in diagnostic-logging and dev-docs/logging.md, and
  replaces the XPath example of a debug record.

- **openspec:** Archive rename-get-attribute-value ([f999534](https://github.com/imbus/robotframework-PlatynUI/commit/f999534e39d1e40c52d4933372e7f4dff1a8da39))

  The new baremetal-attribute-reading spec covers Get Attribute Value and
  its deprecated alias, four specs name Get Attribute Value, and the
  change moves to the archive.

- **openspec:** Record the Windows run of rename-get-attribute-value ([18e2939](https://github.com/imbus/robotframework-PlatynUI/commit/18e2939bef9fd7db817cc910867c01368aaf0b79))

  The full Windows lane passed 146 of 146 on the Windows 11 VM with no
  deprecation warning. An earlier run had two activation failures in
  egui/auto_activate.robot, in steps the rename did not touch; that
  suite passed alone twice, and the next full run was green.

- **openspec:** Record the rename-get-attribute-value implementation ([1b63bd3](https://github.com/imbus/robotframework-PlatynUI/commit/1b63bd314af3c787a78a51e6ea5ac86e5498d81c))

  Tasks 1.1 to 5.4 are done; 5.5, the full Windows lane, is still open.
  The baremetal-waiting Purpose names Get Attribute Value, and the open
  change xpath-document-order uses the new name in its delta and design.

- **openspec:** Propose renaming Get Attribute to Get Attribute Value ([5b08afa](https://github.com/imbus/robotframework-PlatynUI/commit/5b08afaa559b78f3f371823c811604f019c79705))

  Get Attribute returns an attribute's value, not the attribute, and the
  new name pairs it with Wait Until Attribute Value. The change keeps
  Get Attribute as a deprecated alias until it is removed before 1.0 and
  moves the repository's suites, documentation and specs to the new name.

- **openspec:** Archive add-wait-until-attribute-value ([f3dc460](https://github.com/imbus/robotframework-PlatynUI/commit/f3dc46032c90f24895427306fe7ce77b4af2b1e5))

  The baremetal-waiting spec gains the requirement for Wait Until
  Attribute Value, and the change moves to the archive.

- **openspec:** Record the Windows run of add-wait-until-attribute-value ([1b36f4e](https://github.com/imbus/robotframework-PlatynUI/commit/1b36f4e544110897b556073c27ea79e775a65c1b))

  The egui wait suite passed 7 of 7 on the Windows 11 VM, in the updated
  clone and again in a fresh clone built from scratch; a captured
  element that went away failed at once. Task 5.4 records the commands,
  since test-acceptance-windows takes its arguments as the whole
  robotcode command line.

- **openspec:** Record the add-wait-until-attribute-value implementation ([9d53031](https://github.com/imbus/robotframework-PlatynUI/commit/9d5303145a7fff884045a0152676897c673b2afc))

  Tasks 1.1 to 5.3 are done; 5.4, the egui wait suite on real Windows,
  stays open. The design records how the keyword reports a captured
  element whose every attempt raised, and the Purpose of
  baremetal-waiting names the fourth wait keyword.

- **openspec:** Archive atspi-application-level ([3a84c3e](https://github.com/imbus/robotframework-PlatynUI/commit/3a84c3e58f3ed49cc10c8140fa263bf14c160f9e))

  All of its tasks have run, on both Linux lanes and by hand against an
  Avalonia 12 application, so its delta becomes the new main spec
  atspi-application-level: the registry's applications are the
  application level, with the role their object reports, and the
  Application interface classifies nothing below it.

- **openspec:** Archive node-id-is-developer-id ([871d857](https://github.com/imbus/robotframework-PlatynUI/commit/871d85770ac6a59649bfb83ebb66b647b37b4bbd))

  All of its tasks have run, on Windows and on the Linux host, so its
  deltas are merged into the main specs: id-attribute is a new capability,
  sidecar-deployment's process-ID requirement returns under its new name
  without the node identifier, and application-process-attributes drops
  the clause that tied a control:Id to the application's name.

- **openspec:** Record the Linux run of node-id-is-developer-id ([cb53eaf](https://github.com/imbus/robotframework-PlatynUI/commit/cb53eafdc4189b84c6824319d56fbf9879638fcc))

  The AT-SPI half, written on Windows, now ran on the Linux host. Against
  the commit before the change, the new egui query cases fail on X11
  exactly as predicted; with it, both pidns runs, the query suite and
  both Linux lanes pass (98 and 99 tests), and the manual check finds no
  @Id on applications and no empty @Id on the desktop.

  A Qt application root reports its application name as its accessible
  id, and QApplication only when it sets none. The proposal, design D2,
  the id-attribute scenario and task 5.4 now say so; the rule itself is
  unchanged.

- **openspec:** Archive snapshot-validity ([5affa37](https://github.com/imbus/robotframework-PlatynUI/commit/5affa375e0b713c1ccb2a5d830a3a5c058315989))

  Its last open tasks were the Linux run, which the Linux gate of
  application-process-attributes already covered on a tree holding this
  change, and the commit step, which was done but not ticked. The change
  is complete, so its deltas become the new main specs
  application-node-validity and ui-context-lookup.
  jab-discovery-containment now cites the archived run notes.

- **openspec:** Record the Windows run of node-id-is-developer-id ([bc703cd](https://github.com/imbus/robotframework-PlatynUI/commit/bc703cd42e01700216d24411b56b6299a9455e18))

  The change behind the previous commit, with its Windows run notes: the
  red states before each fix, the unit tests, the Java live fixture and
  the full lane. The AT-SPI tasks that need the Linux host stay open:
  the X11 red state, the pidns harness, both Linux lanes and the Qt
  check.

- **openspec:** Archive application-process-attributes ([56f5ac2](https://github.com/imbus/robotframework-PlatynUI/commit/56f5ac2a44f0928b54b901103bbfe126eb2d186e))

  All of its tasks have run, on Windows and on the Linux host, so its
  delta becomes the new main spec application-process-attributes.
  node-id-is-developer-id modifies that spec and can only be archived on
  top of it.

- **openspec:** Propose that Id is the toolkit's identifier ([f2d3ef6](https://github.com/imbus/robotframework-PlatynUI/commit/f2d3ef6f874ab56f09058e5953a2ab9f4aee2e58))

  An element's Id is the identifier its toolkit reports for automation,
  taken by source whoever set it, present only with a value, and the same
  through the attribute and the accessor. Application nodes carry none on
  any provider; their identity is @ProcessId.

  Today AT-SPI answers an application's id() with its process ID and
  lists @Id="" on every node without an accessible-id, and UIA gives an
  application node its name, now its process name, as id() and @Id.
  application-process-attributes left that Id alone, as agreed; this
  change removes it and the spec clause that ties it to the name.

- **openspec:** Propose an Avalonia test application ([077a9d5](https://github.com/imbus/robotframework-PlatynUI/commit/077a9d5ec5c0b64ec1aac2dec08d3247e5789b44))

  A blueprint-conforming Avalonia 12 fixture for all three lanes. On Linux
  Avalonia exposes accessibility only on X11, so the Wayland lane's
  compositor starts XWayland for it, which depends on
  fix-compositor-xwayland-startup-hang; that proposal now names this
  change as its first consumer. The fixture's suites prove
  atspi-application-level on a real Avalonia tree in every Linux lane run.

- **openspec:** Record the atspi-application-level change ([906d66a](https://github.com/imbus/robotframework-PlatynUI/commit/906d66a320ba63888603872c5762e2d7eb6f9505))

  The change behind the previous commit, with its run notes: both Linux
  lanes, the unit tests, and a manual check against an Avalonia 12
  application, whose window is now control:Frame under a single
  application node.

- **openspec:** Propose Wait Until Attribute Value for BareMetal ([aaa113a](https://github.com/imbus/robotframework-PlatynUI/commit/aaa113a21d76ade2d341033ee6e0eabdcd5f42cc))

  Get Attribute checks a value once, so suites await asynchronous state
  changes with Wait Until Query <selector>/@Attr. The change proposes a
  separate keyword that waits until one attribute of one element
  satisfies a condition and returns the value it had at that moment;
  Get Attribute keeps reading once.

- **openspec:** Record the Linux run of application-process-attributes ([4f0e771](https://github.com/imbus/robotframework-PlatynUI/commit/4f0e77190d9eb5133c5d9a8d2b057398fcb662bd))

  The Linux-host tasks are done: the egui suite passes on both lanes
  against 3d41929 as a regression guard, and the full gate passes on a
  clean checkout of 009595b. What remains is the Windows lane run (7.1)
  and the lane-only checks of 3.2 and 3.3.

- **openspec:** Record the triage of the deferred logging findings ([009595b](https://github.com/imbus/robotframework-PlatynUI/commit/009595bc3e95442ba3e22742f87e00e9768483c8))

  The maintainer sorted every open entry of the logging review, and each
  of the 206 entries now carries a Status line. 102 are dropped because
  no user would notice them, and 12 are priority A. The follow-ups are cut
  by theme, not by area, and come in the order A2, A1, A4, A3, A5. A
  priority-B entry goes with the A follow-up in the same code, or is done
  when its code is next touched; a strict re-check kept 18 of the 52.
  The functional bugs the review surfaced follow per provider. XPath
  evaluation is not reviewed for logging: its own records exist in debug
  builds only, fn:trace() excepted. A paragraph under the preamble
  states these decisions.

- **openspec:** Record the application-process-attributes implementation ([3315d9a](https://github.com/imbus/robotframework-PlatynUI/commit/3315d9afee1d7522c0eb182c85f348b7a9bb7af7))

  Records what was verified on Windows and what waits for the Linux host
  and the Windows lane, so the open steps are not mistaken for done. The
  artifacts follow what implementation and review found: PID 4 cannot be
  opened without elevation, the old UIA listing ended at an absent Id, AT-SPI
  records its process once per node, and the Linux command line is read
  from /proc because sysinfo trims its arguments.

- **openspec:** Build the 32-bit test window in Windows CI ([3d41929](https://github.com/imbus/robotframework-PlatynUI/commit/3d419297929ccc13640d43686a73e4d079e61ec7))

  The x64 builds cannot see a break that only the i686 target has, so
  without a CI step it would first show in a local lane run. The plan
  has the rust-windows job install i686-pc-windows-msvc and build the
  window. Linux needs no new target: the window never runs there, and
  the cross checks already compile it for Windows x64.

- **openspec:** Build application-process-attributes on the identity ([5a431b4](https://github.com/imbus/robotframework-PlatynUI/commit/5a431b4a81cdcfabd4fd267a1df67a10dd5919d7))

  The plan predated snapshot-validity, which created crates/process and
  gave the three application nodes a recorded process identity. Reading
  through that identity is what makes "a node reports only the process
  it was created for" hold, so the reader now binds to it, and a node
  whose process has ended lists no app: attribute.

  A check against the current code also corrected what the Java agent
  reports today (no start time on Java 8, the jre launcher path), adds
  the pid-0 guard to automatic agent attachment, orders the agent first
  among the providers, and keeps sysinfo as the Linux source.

- **openspec:** Close the review gaps in jab-discovery-containment ([1c3300c](https://github.com/imbus/robotframework-PlatynUI/commit/1c3300c05c31983b3679fe776790465e0e5922d9))

  Two adversarial reviews of the proposal found that its mechanism did
  not deliver what its specs promised. Held windows would lose their
  claim after one busy pass, a stuck window call could name the healthy
  JVM when the bridge asks every JVM, a late-started call would set a
  healthy window aside, and a stall inside the bridge's own message
  handling went unseen. The specs also contradicted java-provider and
  the single-appearance requirements, and several tests could not fail
  before the fix.

  The plan now states attribution in terms of the bridge's behavior,
  measures that behavior before any live assertion, and gives every
  scenario a test that fails today.

- **openspec:** Propose containing JAB discovery against a stuck JVM ([1cf0020](https://github.com/imbus/robotframework-PlatynUI/commit/1cf0020f94f84c9c99091fe1fc9df72771d6a3a3))

  One Java application whose event thread does not answer can take every
  other bridge-served application out of the tree: discovery abandons its
  pass at the first timeout, and calls that only waited behind a stuck
  one are blamed on their own JVM. The lane only stays green because its
  live tests are serialized; users still meet it.

  The change bounds and reports such a stall, stops the misattribution,
  keeps other JVMs' windows once the bridge answers again, and asks the
  bridge only about AWT windows.

- **openspec:** Record the snapshot-validity implementation ([1f09f50](https://github.com/imbus/robotframework-PlatynUI/commit/1f09f5039eeff3361da2803ed3fd221d1bd829d5))

  The Windows side is done and verified: unit, live and Python tests, the
  red-before run of the six new acceptance tests on the old build, and a
  green lane with no warning. The review fixes and the lane's detour
  through the JAB interference are recorded where they happened. Linux
  verification (2.1, 9.3) remains open.

- **openspec:** Archive xdm-snapshot-release ([65e4a4f](https://github.com/imbus/robotframework-PlatynUI/commit/65e4a4f5ea253b6e8e4f546b54faa4484dbd1554))

  Its last open task, the memory measurement, now holds on Windows and
  Linux against an app of our own, with a leaking build as the positive
  control. The change is complete, so its delta becomes the new main
  spec xpath-snapshot, which later changes such as xpath-document-order
  build on.

- **openspec:** Record the Linux snapshot memory measurement ([93cce5c](https://github.com/imbus/robotframework-PlatynUI/commit/93cce5c599b1af069bc813240aab33913cb0609a))

  Run measure_snapshot_memory.py in an X11 session on AT-SPI against this
  change and against 0802be7 as the positive control. The control grows by
  about 2 MiB per discarded snapshot; this change stays below 0.5 KiB per
  evaluation. This completes task 7.3 of xdm-snapshot-release.

- **openspec:** Align gate-uia-window-patterns with the current code ([be4dce0](https://github.com/imbus/robotframework-PlatynUI/commit/be4dce0bae67e3362f84f84b09111162ed12a580))

  4b1fc6c made a refused foreground change a debug record, as the
  diagnostic-logging spec asks, so the planned once-per-episode warning is
  dropped and task 4.2 keeps only the property-id comment fix. With
  window-activation-state archived, window-activation is an existing
  capability: the delta loses its Purpose and task 1.2 its wait.

- **openspec:** Archive window-activation-state ([dd7af02](https://github.com/imbus/robotframework-PlatynUI/commit/dd7af0215cd39fba359976068e77c5f2ca137adc))

  Its four open Windows checks now pass on the current head (19 of 19),
  so the change is complete. Its deltas become the main specs
  window-activation and window-state. The record names the one known gap,
  Bring To Front on an element inside a window under UI Automation,
  which gate-uia-window-patterns closes.

- **openspec:** Archive java-agent-web-start ([38ff511](https://github.com/imbus/robotframework-PlatynUI/commit/38ff5119b02550053efdd7c55a639a949c88039a))

  All 36 tasks are done, so its deltas become the main specs: the agent
  works under a target's security policy, the agent tree and runtime
  cover every AppContext, and the Swing fixture gains its Web Start launch
  modes. Later changes to java-provider now build on the current text.

- **openspec:** Build the 32-bit test process for process attributes ([fce970c](https://github.com/imbus/robotframework-PlatynUI/commit/fce970c4bd970b648454fdf9865aa607e250c99f))

  The architecture scenario ran against charmap.exe, an application that
  ships with Windows and can change with any update, which tests here must
  not rely on. The plan now builds a minimal Win32 window of its own for
  i686, which makes that Rust target a prerequisite of the Windows lane.

- **openspec:** Record the Windows run for window-activation-state ([baa5d62](https://github.com/imbus/robotframework-PlatynUI/commit/baa5d627a80e6fb8d35db6143be06d76e45f1fe4))

  The only proof of its four open Windows checks was results/output.xml,
  which the next lane run overwrites. The run, its code state and the
  tests behind each task are now written down; whether it counts as the
  verification stays the maintainer's call, so the boxes stay open.

- **openspec:** Propose document order for XPath results ([13328fe](https://github.com/imbus/robotframework-PlatynUI/commit/13328fe93c47f08aad60bf57c312d8542444dc44))

  Paths like //X return their matches grouped by tree level, so every
  keyword that acts on the first match can pick the wrong element without
  an error, on every provider. The plan makes the engine follow XPath 2.0,
  with document order and positional predicates per context node, while
  the first match stays cheap. It scopes Java agent ids per view and keeps
  what the PlatynUI.core Locator's index and position mean.

- **openspec:** Propose gating UIA window patterns ([86ab183](https://github.com/imbus/robotframework-PlatynUI/commit/86ab183921d992f9509eee5d26a174dc1bb0a87d))

  UI Automation hands out Activatable and the other window patterns on
  every element, so bringing a window to the front focuses the element
  instead: it moves the focus inside the application and fails on
  disabled elements. The plan gates those patterns on the window surface,
  activates the root window through the injected window manager, and adds
  a testkit check that advertised and served patterns agree.

- **openspec:** Record the implementation of xdm-snapshot-release ([78bbe61](https://github.com/imbus/robotframework-PlatynUI/commit/78bbe6171678d7dc2e5ed269ff442daf5ae3ee01))

  The plan changed while it was carried out: the review found cases the
  design missed, measuring found a second leak in UI Automation, and the
  egui test app needed an accessible container. Proposal, design, spec
  and tasks now say what was built, and record the test runs and
  measurements that are still open.

- **openspec:** Revise application-process-attributes ([0802be7](https://github.com/imbus/robotframework-PlatynUI/commit/0802be7cf455dd90596e39a481844cf7af193b7a))

  Testers read process metadata in selectors, Get Attribute, the Python
  Application, the CLI and the Inspector, so the plan now treats wrong or
  provider-specific values as user-facing defects and documents them for
  users. At 0.x the shape changes are behavior changes, not breaking ones.
  It also fixes the Python Application, derives the node name from the
  process name, and aligns with snapshot-validity's crates/process.

- **openspec:** Archive logging-concept ([37b6322](https://github.com/imbus/robotframework-PlatynUI/commit/37b6322daffd07651afca606e6c95d407327d109))

  Every task is implemented and verified, on Linux and now on Windows, so
  its deltas become the main specs: diagnostic-logging is new, and
  jab-provider, java-app-classification, native-logging and
  runtime-session-config carry the changed requirements.

- **openspec:** Propose snapshot validity for app nodes and core ([dee1f57](https://github.com/imbus/robotframework-PlatynUI/commit/dee1f5795262d6dc6f6097c6cf2ea416ac0d39b9))

  The snapshot model relies on is_valid and on callers discarding the
  snapshot after a miss. The synthetic application nodes of UIA, JAB and
  the Java agent report valid forever, so a root pinned to an ended
  application is never looked up again, and PlatynUI.core never discards
  the snapshot, so its retries ask the same one again.

- **openspec:** Propose releasing XPath snapshots ([33de88c](https://github.com/imbus/robotframework-PlatynUI/commit/33de88c85712bdce0a28e0d4207b44abbcde0384))

  Every snapshot the runtime discards stays in memory, because the XPath
  wrappers of a parent and its children hold each other; measured on
  Windows at up to 15 MiB per query under a large window. Ancestors of
  returned nodes survive only through that leak, so the change first has
  providers keep the parent of each listed node, then breaks the cycle.
  It also writes the snapshot model down as a spec for the first time.

- **openspec:** Record the Windows verification of logging-concept ([f4b7328](https://github.com/imbus/robotframework-PlatynUI/commit/f4b7328fd8666d6adcb2f62a67be0bf8416dc9cb))

  The last open tasks needed a Windows machine. They now pass there, so
  nothing moves to a follow-up change, and the change can be archived.
  Task 6.3 records what was checked and a memory leak in the XPath cache
  that turned up on the side and is outside this change.

- **openspec:** Record the state of the logging concept ([167c791](https://github.com/imbus/robotframework-PlatynUI/commit/167c79148a9b3f9b7eb3bb34b4a5ee1a7b4d27ba))

  Every task is implemented and committed, and verified on Linux and, by
  cross-checks, for macOS. Open are only the checks that need a Windows
  machine: the Windows parts of the configuration checks, the Java agent
  and the JAB and UI Automation changes. The design now says that an
  element description escapes a name as Rust's Debug does, leaving
  single quotes as they are.

- **openspec:** Propose a logging concept for the library ([96bc8dc](https://github.com/imbus/robotframework-PlatynUI/commit/96bc8dc720eb3dc82804d6d319f2ec5f24454996))

  Since native records reach the Robot Framework log, PlatynUI's logging
  is user interface, and a project-wide review found it not ready:
  warnings that repeat on every poll, failures reported twice, failures
  that stay silent, levels without an agreed meaning, and rules that
  cover only Rust.

  The proposal writes one concept for Rust and Python in
  dev-docs/logging.md: one meaning per level, log or return, once per
  episode through a transition latch in platynui-core, one element
  description, message style and field names. One level knob serves the
  CLI, the Inspector and the Python extension: a single level lowers
  PlatynUI's own modules, error and off apply to every module, and
  RUST_LOG is the only source of directives. Keywords log one DEBUG line
  per action, never repeat the text they type, and accept Robot
  Framework's Secret. Each component reports the configuration it cannot
  use. The change fixes the review's high-priority findings for the
  library and modifies native-logging, runtime-session-config,
  jab-provider and java-app-classification.

  The development tools stay out of scope. The review's other findings
  are kept in review-findings.md, each marked where this change
  implements or supersedes it.

- **openspec:** Archive the native log bridge change ([6ffef13](https://github.com/imbus/robotframework-PlatynUI/commit/6ffef137d460e59b519ac7a23f967662be79d522))

  The change is implemented and committed, so its delta becomes the new
  main spec `native-logging`. It states how diagnostics of the native core
  reach Python `logging` under `platynui.native` and the Robot Framework
  log of the keyword they belong to, that only warnings and errors are
  produced unless `native_log_level` or the environment asks for more,
  that logging never blocks or deadlocks native code, and that records
  waiting for delivery are bounded and their loss is reported.

  The change directory moves to the archive unchanged, delta spec
  included, so the decisions behind those requirements stay readable.

- **openspec:** Archive the Wayland sidecar capabilities change ([463c7ca](https://github.com/imbus/robotframework-PlatynUI/commit/463c7ca0b842f14ff0cece0a042011d79f46eeb3))

  The change is implemented and committed, so its delta becomes the new
  main spec `wayland-compositor-detection`. It states how the Wayland
  backend decides which compositor it is talking to without relying on
  the compositor's process being visible: the control-socket handshake
  first, the session environment second, the peer process only for
  foreign compositors. It also covers an identified PlatynUI session whose
  control socket does not answer failing by name instead of being
  downgraded, gated actions such as highlight reporting unavailable
  instead of success, a substituted window geometry being logged once per
  window, and one identification record per initialization.

  The change directory moves to the archive unchanged, delta spec
  included, so the measurements and the decisions behind those
  requirements stay readable.

- **openspec:** Propose bridging native logs into Robot Framework ([3ebc734](https://github.com/imbus/robotframework-PlatynUI/commit/3ebc734a39c0a92e7a469198c6da453deb3aebed))

  The native extension installs no tracing subscriber, so every
  diagnostic the Rust core emits is dropped when PlatynUI runs from
  Python: the window-bounds substitution warning, the compositor
  identification record, the debug records runtime-session-config
  requires, the output of fn:trace(). The proposal forwards them to
  Python logging and the Robot Framework log.

  The emitting thread only queues a record. Records are delivered on the
  calling Python thread after runtime calls and around every PlatynUI
  keyword, because Robot Framework drops messages from other threads and
  native calls hold the GIL while they join threads that log. Native
  warnings become RF warnings. Only WARN and ERROR are produced by
  default; a native_log_level import argument and the CLI's environment
  variables lower the floor.

- **openspec:** Archive the X11 window-owner identity change ([b3e0043](https://github.com/imbus/robotframework-PlatynUI/commit/b3e0043ead5a4f97f7af6d7e5abaef4ca5757e2f))

  The change is implemented and committed, so its delta moves into the main
  `element-at-point` spec. The requirement that hit-test excludes the host
  process's own UI now says how the X11 path establishes ownership: a window
  is the runtime's own only when it reports the runtime's PID and the X
  server attributes it to the same process as the runtime's own connection;
  a window the server attributes elsewhere, or cannot attribute at all, is
  resolved; a server that cannot be asked keeps the previous comparison and
  reports it once per connection. The PlatynUI compositor keeps its own rule
  from `compositor-client-identity`, and Windows keeps excluding in the
  provider. New scenarios cover the sidecar collision, WSLg and a runtime in
  a container on the host's display.

  The change directory moves to the archive unchanged, delta spec included,
  so the measurements and the decisions behind that requirement stay
  readable.

- **openspec:** Propose one contract for application process attributes ([56b50dc](https://github.com/imbus/robotframework-PlatynUI/commit/56b50dcfd49b94a345ca51e0937e71e1f4e7ab07))

  Every provider that builds application nodes decides on its own what the
  process attributes mean, when they are present and what they say when a
  value cannot be read. Windows UIA and JAB answer placeholders or the
  automation host's architecture, and the Java agent reports its facts in
  another namespace and format.

  The new change `application-process-attributes` defines one contract for
  all providers, including macOS and any later ones: every process attribute
  is optional and reported only when determined, never substituted; fixed
  names, namespaces and value formats; which platform supplies which
  attribute; and a process ID of 0 identifies nothing. Its design puts one
  process reader per platform into a new crate that every provider calls,
  reads the Windows architecture from GetProcessInformation with an
  IsWow64Process2 fallback, and leaves the Java agent unchanged. The tasks
  start with acceptance suites and were reviewed for coverage, feasibility
  and order.

- **openspec:** Archive the AT-SPI process-identity change ([9224d3e](https://github.com/imbus/robotframework-PlatynUI/commit/9224d3e31d2d94f149477a74d59dad66993ef109))

  The change is implemented and committed, so its delta specs move into the
  main specs. `sidecar-deployment` becomes a new capability: PlatynUI
  automates an application in a foreign PID namespace in the supported
  topology and names its prerequisites; process identity is decided once per
  bus connection from the daemon's view of the provider's own connection; an
  unresolved identity never matches; own-process exclusion is applied
  consistently, and the hit-test leaves window ownership to the window
  system; an application reports the process ID its own environment knows,
  and process-table data only through a process ID valid in the runtime's
  namespace and only as far as it was actually read; only definitive
  identity answers are remembered; and a native window is correlated with an
  application only through a process ID both sides can express.

  `atspi-event-driven-tree` now says when the popup watcher hides the
  provider's own popups: only once the provider has established its own
  identity on the watcher's connection. A foreign application's popup is
  surfaced even when it has not.

  The change directory moves to the archive unchanged, delta specs included,
  so the measurements and the decisions behind those requirements stay
  readable.

- **openspec:** Archive the compositor client-identity change ([519d9da](https://github.com/imbus/robotframework-PlatynUI/commit/519d9daae72591966ba6edc956d832df8da248cb))

  The change is implemented and committed, so its delta spec becomes the main
  spec of a new capability: `compositor-client-identity`. It states that a
  Wayland client's process identity is established when its connection is
  accepted and never re-derived while serving a request; that no control-socket
  command may fail, hang or end the session because a client cannot be
  identified; that an unknown process is reported as JSON `null` and never as
  `0`, whatever its source, including a `_NET_WM_PID` an X11 client declares as
  `0`; that an XWayland window otherwise reports the value its client declared,
  never the credentials of XWayland's own connection; that the window at a
  point is never a window of the process that asked, excluded only on a
  positive match of two identities the compositor established itself; and that
  a restrictive protocol policy denies a client it cannot identify without
  ending the session.

  The change directory moves to the archive unchanged, delta spec included, so
  the measurements and the decisions behind those requirements stay readable.

- **openspec:** Simplify the AT-SPI identity decision to one question ([bd12556](https://github.com/imbus/robotframework-PlatynUI/commit/bd12556202a5df12c4a1dd9781d65ffbb2d3a73b))

  The identity chain had four modes — process-fd, translated-descriptor,
  process-id, none. It now has two outcomes, decided by one question asked
  once per bus connection: does the daemon report our own process ID for our
  own connection? Equal means local numbering, so its numbers are values in
  our namespace and may be compared with ours and read through `/proc`.
  Anything else — no number, a `0`, an "unknown", a number that is not ours —
  means no identity: no own-process check, and no reported number treated as
  locally valid.

  What that removes is the process-descriptor chain. It is recorded in D1 as
  a measured alternative that was not taken: the inode comparison needs an
  `fstatfs` check against `pidfs` to survive kernel 6.8, where every pidfd is
  an anonymous inode with one shared inode number, so the naive comparison
  claims every application as our own and empties the tree on a stock Ubuntu
  24.04 desktop; avoiding that needs a second branch through
  `/proc/self/fdinfo` and a descriptor lifetime on the enumeration path. In
  the supported topology it buys close to nothing — a runtime in a sidecar has
  no user interface on the application's display, and the `app:*` block stays
  absent there either way. D2 and D4, which described the chain, are withdrawn
  rather than renumbered, so every cross-reference from the sibling changes
  keeps its number; the same goes for task section 1 (the descriptor probe)
  and task 2.3.

  `@ProcessId` keeps reporting the number the daemon gives, whatever the
  outcome, because it is only ever compared with the display server's view of
  the same application — never with ours. Without it the sidecar topology
  would lose the one correlation that works there.

  The prerequisite boundary is now stated as two independent axes instead of
  one named deployment: whether the daemon can see the application decides
  whether that application has an identity at all, and whether it can see the
  runtime decides whether the provider has one of its own. The supported
  topology is the first without the second; a daemon in a namespace of its own
  loses both, and a daemon that shares the runtime's namespace but not the
  application's loses only the application's. The local harness gains that
  second arrangement, where the provider keeps its identity while an
  application has none.

  The decision needs no syscall, no `/proc` read and no new dependency, so the
  planned `rustix` dependency is gone with it. 30 tasks become 27.

- **openspec:** Record the unsupported three-namespace topology ([056e68c](https://github.com/imbus/robotframework-PlatynUI/commit/056e68c0e0f2ff1ac6a8598586a17cfa5276bf4c))

  `atspi-process-identity` requires the accessibility bus daemon to run in
  the application's namespace, so that it sees the application even though it
  cannot see the runtime. A deployment that gives the daemon a namespace of
  its own — application, daemon and runtime in three — violates that
  prerequisite wholesale: the daemon can number neither side, so no
  application carries a process-ID attribute and what the capability
  specifies as a per-application limitation holds for every application on
  that bus.

  The spec now names that boundary and what it costs: no element resolved at
  a point, failing window-management operations where the process id is the
  window system's only key, and no popup geometry under the PlatynUI
  compositor. It also states that nothing asks PlatynUI to detect such a
  deployment. The window-state attributes are deliberately not listed: they
  read `False` for a window that cannot be resolved, which the
  `window-state` capability already requires.

  The proposal's claim about BareMetal's `/app:Application[@ProcessId=...]`
  locator is qualified accordingly: it keeps its meaning where runtime and
  application share a PID namespace, which is every desktop lane, and matches
  nothing where the daemon can resolve no peer.

- **openspec:** Propose evaluating PID-namespace tests in CI ([0ccf9b2](https://github.com/imbus/robotframework-PlatynUI/commit/0ccf9b269ad3bdb7732751b156f2cb835bf461cb))

  The sidecar changes land their namespace tests as local, ignored
  harnesses. Whether a GitHub-hosted runner can run them hinges on one
  unmeasured fact: whether unprivileged user namespaces can be enabled
  there. This change measures that on a throwaway branch against a
  written acceptance bar and records the outcome either way, without
  choosing or pinning runners.

- **openspec:** Propose PID-namespace sidecar support ([4fa5d65](https://github.com/imbus/robotframework-PlatynUI/commit/4fa5d65971fbc05ac28ab947e3fa32fc8d8bf374))

  Running the runtime in a sibling container without a shared PID
  namespace breaks every place that compares a PID across the boundary.
  PR #5 reported the AT-SPI half of it; reproducing it in podman pods
  showed three more, and that PR #5's own comparison drops foreign
  applications on dbus-daemon <= 1.14 and dbus-broker up to 37.

  Four changes, in implementation order:

  - fix-compositor-foreign-pidns-clients: the compositor captures client
    identity at accept time instead of aborting on an unresolvable peer,
    reports an unknown PID as null, and skips the caller's own windows in
    window_at_point.
  - atspi-process-identity: the AT-SPI provider decides once per bus
    connection how it recognises itself (process fd, translated
    descriptor, process id, or not at all), reports the daemon-view PID as
    @ProcessId, and reads app:* only through a locally valid PID.
  - wayland-sidecar-capabilities: the compositor is identified over its
    control socket instead of by peer PID, highlight stops reporting
    success it did not have, and a substituted window geometry is logged.
  - x11-window-owner-identity: own windows on X11 are identified through
    the X server's view of our connection (XRes) instead of a
    client-reported _NET_WM_PID.

- **openspec:** Measure the agent's X11 window id on Linux ([797edbc](https://github.com/imbus/robotframework-PlatynUI/commit/797edbcda07e71cb6503a2b2e8fdd7cfedf51265))

  java-provider-linux left decision 1(a) waiting on a measurement, and the
  agent turns out to be far enough along to answer it today: the in-process
  read works on Temurin 8, 21 and 26 with no launch flags at all, so the
  `--add-opens` matrix that task budgeted for does not exist.

  The windowing half stays open — all three ran under WSLg, which cannot
  identify its own X server from inside the distro and synthesises no input,
  so it can host this measurement but not the acceptance lane.

- **openspec:** Source Linux attach candidates from the windowing system ([1b6e3ad](https://github.com/imbus/robotframework-PlatynUI/commit/1b6e3ad34907ce12f3bf20797e4aaaecf7d7c05d))

  The change assumed automatic attachment would come along for free on
  Linux. It would not have: JavaProvider offers the agent to the JVMs behind
  the Java windows a pass saw, and that candidate list is filled ONLY by
  backends that enumerate windows — which means JAB, on Windows. Gating JAB
  out (task 2.1) removes the list entirely, so auto_attach would stay on by
  default and never be able to fire, and only JVMs launched with -javaagent
  would ever be served. That is java-provider's existing "Automatic
  attachment to detected JVMs, on by default" requirement inherited unmet
  on the platform this change adds.

  So Linux gets a candidate source: the router keeps the WindowManager it
  already receives, and fills the list from native top-level enumeration
  with each window's owner resolved by the rule dev-docs/java-toolkits.md
  already settles — X-Resource XResQueryClientIds, which the X server
  derives from socket credentials, with EWMH _NET_WM_PID only as a
  corroborated fallback. Attaching into a process a window merely claims is
  not a cosmetic error.

  Two consequences worth stating rather than discovering later. The
  WindowManager trait change is no longer contingent on a measurement, so
  this change definitely touches platynui_core; design 1 narrows to whether
  the agent's in-process X11 id is used IN ADDITION, for identity. And the
  condition for using the window-derived source is which backends exist —
  declared statically on the trait — not whether a pass produced candidates:
  the latter is also true on Windows whenever JAB sees no Java window, which
  would switch Windows onto the new source behind a Non-Goal, invisibly,
  since it is masked today only by platform-windows not implementing the
  enumeration.

  Also records what java-agent-web-start means here: with no JAB fallback on
  Linux, an application the agent cannot see is an application that is not
  there at all, so the AppContext fix matters more on this platform than on
  the one it was found on.

- **openspec:** Propose Web Start support for the Java agent ([fbc7d2e](https://github.com/imbus/robotframework-PlatynUI/commit/fbc7d2e257a46076d46b586b6e5d392dc48f7d79))

  Java Web Start is one of the reasons the agent exists — the java-agent
  capability names "scripts, installers or Web Start" as why attaching into
  a running JVM is the primary path. Measured against a real OpenWebStart
  1.14 application on Java 8, the agent reached none of it, for two
  independent reasons, both reproduced without OpenWebStart afterwards:

  - The JNLP SecurityManager kills it before it publishes anything.
  - Even fixed, it sees no windows: Window.getWindows() and
    EventQueue.invokeLater resolve against the CALLING thread's AWT
    AppContext, and IcedTea-Web gives every application its own while the
    agent's threads live in the system one. Measured in the target: 2
    contexts, the application's JFrame in the one the agent is not in.

  Neither is incidental to OpenWebStart. The first hits any target with a
  restrictive policy, the second any application in its own AppContext —
  Web Start and applets by construction.

  The security-policy half is implemented (previous commit); this records
  what it is, why Boot-Class-Path unconditionally rather than conditionally
  (under -javaagent the agent runs before the security manager exists, so
  the condition cannot be evaluated when the decision must be made), and
  what it costs the target (class-data sharing off on JDK 9+, disclosed
  rather than avoided). Its tests do not exist yet, and every one of these
  fixes fails silently, so nothing here is finished.

  Two constraints are recorded where they will be acted on rather than
  here. Bootstrap-defined classes can only resolve what the boot loader
  defines: java.desktop is such a module, so Swing is unaffected, but
  neither javafx.* nor org.eclipse.swt.* is, and both adapters must reach
  every type through a loader taken from the application side. Both designs
  were written assuming direct references, so provider-java-javafx
  decision 3 and provider-java-swt decision 6 are corrections to them.

  The policy regression fixture has an expiry date: JEP 486 permanently
  disabled the security manager in JDK 24, where such a JVM cannot start at
  all. That is in the spec text, not only in the design, because the spec
  is what survives archiving.

- **openspec:** Propose fix-compositor-xwayland-startup-hang ([58d03c0](https://github.com/imbus/robotframework-PlatynUI/commit/58d03c06957e1f623d63d2c23cb43e9c28fe9cc4))

  A compositor started with --xwayland holds back readiness and its child
  program until XWayland reports ready, and no failure path ever releases
  them: a missing or non-executable Xwayland, a server that exits or stays
  silent before it is ready, or a window manager that fails to start leave
  the session running with no readiness and no result.

  The change makes every such case a fatal startup error (ERROR log with
  the server's own output, no readiness, no child, exit 1 with or without
  --exit-with-child), bounds the wait with a new --xwayland-timeout, starts
  the X11 window manager before readiness, contains a mid-session XWayland
  termination, and makes the IPC status report whether XWayland is usable.
  The new compositor-session-lifecycle capability also records the exit
  code contract from ae5f614.

- **openspec:** Archive acceptance-lane-wrapper-sessions, sync specs ([a20a391](https://github.com/imbus/robotframework-PlatynUI/commit/a20a39110d03e33c7abe6a6feb5218ed6e6f617f))

  All 18 tasks done and verified, including the manual VS Code check, so
  the change moves to the archive and its delta is synced into the
  acceptance-lane-selection spec.

  "Lane entry points choose the matching profile" is removed rather than
  modified: its scenarios -- the XDG_SESSION_TYPE profile default, the
  argument pass-through and the fallback to `real` -- describe exactly the
  behavior the change takes out. Its successor "Lane profiles establish
  their session" carries the wrapper contract. "Lane profiles select by
  excluding foreign platforms" gains the rule that no profile narrows
  `paths`, with a scenario for longnames that stay stable across profiles.

- **openspec:** Note the mock-only scenario in window-activation-state ([cfb1479](https://github.com/imbus/robotframework-PlatynUI/commit/cfb14795ea7a109d904148b14d1d7d7a22d03809))

  Bringing an element of a minimized window to the front is only verifiable
  against the mock: real toolkits may withdraw a minimized window's contents
  from the accessibility tree, so the acceptance lane brings the window
  itself to the front instead.

- **openspec:** Propose window-activation-state ([0be4155](https://github.com/imbus/robotframework-PlatynUI/commit/0be4155c1037611e262114a40e96da6c947e5f57))

  Activating a window must not change its size: a minimized window comes
  back in the state it was minimized from, a maximized one stays maximized,
  and bringing an element to the front no longer restores first. Alongside,
  the WindowManager learns to report a window's state so AT-SPI and JAB can
  expose IsMinimized, IsMaximized and IsTopmost.

  Tasks reflect the Linux implementation; the Windows-only checks (UIA
  activation, JAB attributes, the Windows lane) are still open.

- **openspec:** Archive java-agent-table-rows ([c98cc6e](https://github.com/imbus/robotframework-PlatynUI/commit/c98cc6e9d4a2be7b3f970c17dbe4805516c7fae1))

  All 31 tasks done and verified, so the change moves to the archive and its
  requirement is synced into the main spec. The sync is purely additive --
  32 insertions, zero deletions -- because the row level does not
  invalidate anything already specified: the existing scenario "A Swing
  table cell has full fidelity through the agent" still holds, one level
  deeper, and no main spec pinned the fixture table's size.

  The artifacts carry two things learned while implementing rather than
  while planning: design decision 5, which clips geometry to the viewport
  after the grown fixture showed off-view content publishing bounds outside
  the window, and the task group that records it along with the measured
  cost of the bigger fixture on the bridge (ten full walks, 18s).

- **openspec:** Propose hierarchical trees for the Java agent ([423ac6c](https://github.com/imbus/robotframework-PlatynUI/commit/423ac6c18483358f49c1a3944861503d86c49732))

  A JTree served by the in-JVM agent is one node deep. Measured against a
  live JDK 21 tree: the agent reports one child -- a Label named after the
  root -- and nothing beneath it. A JTree's only child component is an
  invisible CellRendererPane, so the walk falls through to the accessibility
  view, and childrenOf then treats every virtual child as a leaf, a rule
  written for table cells.

  The accessibility view is not the answer either, and the proposal records
  the measurements: a collapsed branch announces a child with no name, no
  role and no bounds; getAccessibleSelectionCount answers 0 while
  getSelectionPath is correct, because the JDK only tests the root; and a
  node's role comes from the cell renderer, so the provider's TreeItem rule
  never fires and nodes map to control:Label.

  The central decision is which nodes exist: only realized ones, with a
  collapsed node advertising CanExpand and reporting no children. Identity
  is the TreePath rather than a row index, and the change carries a TreePanel
  fixture stage, since the Swing fixture has never contained a tree.

- **openspec:** Propose row-structured tables for the Java agent ([3e3dcae](https://github.com/imbus/robotframework-PlatynUI/commit/3e3dcaef76610dc45b647b2218396e520ed01b1d))

  A Swing table served by the in-JVM agent is one flat, row-major list of
  cells: the fixture's 4x3 JTable has twelve direct children and no rows.
  That shape is inherited from the Access Bridge, which only ever offered
  the accessibility view's flat child list -- it is not a PlatynUI model.
  provider-atspi already maps TableRow and provider-windows-uia maps
  DataItem, so rows bring Java into line rather than diverging.

  The design settles the cost in the open rather than as a footnote: the
  same table then has different shapes depending on which backend serves
  it, nested through the agent and flat through the bridge. It also
  resolves the implementation constraint this creates -- AccessibleSelection
  on a JTable reports selected cells, so SwingTree.childAt can no longer
  align accessible indices with direct children and the selection is
  re-derived from the table's own model instead.

- **openspec:** Re-ground fix-jab-hit-test-virtual-children on the bridge path ([91387bb](https://github.com/imbus/robotframework-PlatynUI/commit/91387bbe0bbc9a8172652a0c44253ac140b376f3))

  Its premise no longer holds as written. "The Inspector's live picker
  cannot reach a JTable, its cells, or its column headers" was true of the
  only Java channel there was; since `provider-java-swing` the in-JVM
  agent is the preferred backend and resolves exactly those — its
  hit-test chain ends at the cell, and headers carry real bounds.

  The change is not obsoleted, it serves a different population: the
  bridge is the floor for JVMs the agent cannot reach — attachment
  switched off, the delivery package deliberately not installed (since
  installing it *is* the consent for in-JVM instrumentation), or injection
  refused. Rewritten to say so, so nobody picks it up believing they are
  fixing the default path.

  Two things stay untouched by the reframing, and both are called out: the
  technical analysis (isSameObject cannot match renderer-aliased cells or
  freshly-allocated header wrappers), and the second half of the fix —
  resolving to the deepest matched ancestor rather than a parentless node
  is plain robustness of the JAB reveal, independent of tables.

  Also fixed: `add-jab-interface-attributes` was listed as active; it was
  archived on 2026-07-20. And the open question about covering JTree here
  is answered — the fixture has no JTree at all, so that stage belongs
  with the work that wants it rather than inside a two-function hit-test
  fix.

- **openspec:** Archive provider-java-swing, propose verify-display-scaling ([f481ac1](https://github.com/imbus/robotframework-PlatynUI/commit/f481ac117e262bc92ac3fe87dbef335ee918b65e))

  Archives the change and syncs its four requirements into the
  `java-provider` capability: the agent-backed tree, node validity being
  answered rather than assumed, keyword-free backend selection, and
  automatic attachment on by default.

  The HiDPI verification moves out into `verify-display-scaling` instead
  of being archived as a checked box over an unmeasured claim. It could
  not be done here — the host reports 96 DPI and JDK 17 was absent — and
  the new change is deliberately wider, because the blocker is a *scaled
  display* and one covers all three providers.

  The finding that motivated widening it: the JAB backend's per-window DPI
  transform is documented as identity at 100%, so that entire calibration
  mechanism has never executed on any machine we develop or test on. No
  test in the repository exercises a scale other than 100%, and every
  position PlatynUI reports is physical desktop pixels that pointer input
  aims at — a systematic scale error makes every click land wrong while
  the tree still looks correct.

  Also records what was deliberately left out and why: row-grouped tables
  and tree hierarchies are a core-vocabulary decision rather than a Java
  one, and the fixture has no JTree to develop them against.

- **openspec:** Split the Linux bring-up out of provider-java-swing ([e5c02a5](https://github.com/imbus/robotframework-PlatynUI/commit/e5c02a58670b89da0409b987f3bea30aee35b164))

  Reviewing what provider-java-swing's Linux lane actually needs turned up
  three gaps that its task list read as suite wiring:

  - provider-atspi enumerates the accessibility *registry*, not windows, so
    it only ever sees applications that registered on the a11y bus. A Swing
    process without java-atk-wrapper never registers. That absence is the
    premise of serving Swing through an agent — there is nothing to
    deduplicate — but it also means AT-SPI contributes no window node the
    agent backend could attach to. The node has to come from somewhere.
  - crates/provider-java is cfg(windows) in its entirety, because JAB was
    its only backend when unify-java-provider created it.
  - platform-linux-x11 reports java_classifier: None, so no JVM
    classification exists on Linux at all.

  None of that is Swing-specific, and the adapter that needs it most is
  JavaFX, whose whole reason to exist is that JavaFX has no native
  accessibility on Linux. Carried inside the Swing change it would be built
  once for Swing and again for the next adapter, so it becomes
  java-provider-linux — named for what it is, the Linux bring-up of the
  provider rather than of a toolkit.

  The split also unblocks the Swing change rather than only shrinking it: the
  Linux question was written down as an OPEN design decision, and an open
  decision blocked all eighteen of its tasks, including the fifteen that have
  nothing to do with Linux. It is now fully decided and applicable; the open
  question moved to the change that owns it.

  The Linux change leads with a measurement, not an implementation. Whether
  the agent can read its window's X11 id in-process (sun.awt.X11.XBaseWindow)
  across JDK 8/17/21, on bare X11 and under XWayland, decides between "the
  agent is the single source, no new platform capability" and "WindowManager
  gains an enumeration answered from _NET_CLIENT_LIST". That task gates the
  rest so neither branch gets built speculatively. Two thirds of the problem
  is already solved: JVM discovery is handshake::discover(), which is
  platform-neutral, and a JVM's windows are known from inside it.

  Also recorded, because the split makes them load-bearing rather than
  incidental:

  - provider-java-javafx and provider-java-swt now depend on
    java-provider-linux; without it the provider does not run there at all.
  - For SWT on Linux the duplicate problem is real, unlike Swing: SWT is
    GTK-native, so it does register on the a11y bus and is visible to
    provider-atspi — which consults no window claims, the single-appearance
    guarantee being Windows-only today. Left as an open point on the SWT
    change, where it is due.
  - provider-java-swing decision 5's PID+geometry fallback is Windows-shaped
    on purpose: it presumes a native window list, and X11 has none to match
    against.

- **openspec:** Archive unify-java-provider and sync its specs ([a8b535a](https://github.com/imbus/robotframework-PlatynUI/commit/a8b535abbfa27dbfa8f5ab11f8a2760b940c768b))

  All 11 tasks are implemented and the behavior-preservation gate held: the
  Windows acceptance lane passes with zero changes to the Robot suites.

  New capability `java-provider` — one registered Java provider routing to
  toolkit backends. Its Purpose records why the shape matters rather than
  just what it is: a single claimant is what keeps window claims a boolean
  question instead of rank-based ownership between competing Java providers.
  `jab-provider` and `jab-hit-test` are modified in place: JAB is a backend
  now, its config keys moved under `providers.java.jab.*`, and its
  enablement-diagnostic requirement names the router as the emitter — the
  backend reports what it cannot serve, because whether a window is truly
  unreachable is a question only the side that knows all backends can answer.

  Two spec deltas were added during implementation, not planned: the
  enablement diagnostic (its emitter moved) and hit-test single-appearance
  (the claimant is the Java provider now). Without them archiving would have
  left two statements in the living specs that the code had already
  contradicted.

  What the change deliberately did not do is recorded with it: selecting
  between two backends over the same window. The claim rule is complete —
  one backend cannot overlap itself — but the selection needs an
  `Enumeration` shape that maps nodes back to windows, and the criterion
  that drives it is the agent-presence signal. That work is
  `provider-java-swing` task 3.1, and the constraint is written down there
  rather than here, so it is found where it will be done.

- **openspec:** Archive java-agent-core and sync its specs ([5becba9](https://github.com/imbus/robotframework-PlatynUI/commit/5becba96b5ea418853b3f1d8c23395302340b1f2))

  Adds the java-agent capability spec (injection into a running JVM,
  handshake-file rendezvous, bounded multi-client runtime, opt-in package
  delivery, quiescence) and appends the agent-presence signal to
  java-app-classification.

- **openspec:** Record what java-agent-core measured ([588a36d](https://github.com/imbus/robotframework-PlatynUI/commit/588a36d08c045cd58cb296594f90b0802b048c55))

  All 19 tasks are implemented; the walking skeleton's job was to confirm
  the decided approach, and it corrected it in two places.

  Verification results replace the "mechanics to confirm" list:

  - The native attach works on Temurin 8 and 21, including the unsafe
    Windows leg. jattach is not vendored — the escape hatch stays unused.
    Two limits are reported loudly instead of worked around: x86-64 only,
    and no 32-bit JVM from a 64-bit process.
  - New: the `load` reply differs by JDK. Java 8 answers a bare integer,
    JDK 21 answers `return code: 0`. The first implementation rejected a
    successful JDK 21 attach as a protocol error.
  - JEP 451 is confirmed, and more precisely than assumed. JAVA_TOOL_OPTIONS
    does control EnableDynamicAgentLoading, which is the load-bearing
    claim; only the cosmetic warning suppression needs the real command
    line. A blocked load surfaces as agent-refused, not attach-failed.
  - The Windows owner-only ACL is real rather than inherited luck: verified
    under a parent granting Everyone full control, and by loosening an
    existing directory. Hardening therefore runs on every start.

  Refinements folded back: an empty toolkit set right after a `-javaagent`
  injection is normal, not a bug, so detection re-runs and republishes; a
  toolkit appearing is the first real producer of the UI-generation counter.

  Two items stay open and are stated as such: the Unix attach leg
  cross-compiles clean but has never run against a live JVM, and wedging a
  toolkit thread end-to-end needs a real dispatcher, which arrives with
  provider-java-swing.

- **openspec:** Split the agent foundation out of provider-java-swing ([ea2f817](https://github.com/imbus/robotframework-PlatynUI/commit/ea2f817f77749579f58d88f882ecb21d87560f61))

  provider-java-swing carried two different things — getting an agent into
  a JVM, and turning its answers into PlatynUI nodes — which made it the
  largest open change at 27 tasks and forced an illogical order: start it,
  switch to unify-java-provider, come back. The seam shows in its own spec
  delta: two of its five requirements never mention a tree.

  - java-agent-core (new): the Gradle product project, the agent JAR
    skeleton (entry points, wire, handshake file, element registry,
    threading), the native Rust attach transport, the new crate
    crates/java-agent with the wire-level client, and delivery via the
    platynui-provider-java wheel and the platynui.providers entry point.
    Ends where the adapters begin: the agent is in the JVM and answers.
    It surfaces no PlatynUI node and depends on nothing open.
  - provider-java-swing: keeps the mapping layer and the Swing/AWT
    adapter, down to 19 tasks; decisions renumbered 1-5.
  - javafx/swt: now depend on both, which is what they always needed —
    the infrastructure, not the Swing adapter.

  Order becomes linear with no interleaving: java-agent-core and
  unify-java-provider (independent of each other) before provider-java-swing.

  Review findings folded in while splitting:

  - auto_attach was specified in the foundation, which has no provider and
    no window enumeration and therefore cannot implement or demonstrate it.
    The mechanism and its availability gate stay in java-agent-core, the
    routing policy moves to provider-java-swing as its own requirement.
  - The Linux half of the Why was never verified: the Swing suites are
    tagged platform:windows because JAB is Windows-only, so nothing proved
    Swing is reachable on Linux without java-atk-wrapper. Added as an
    acceptance task with its own X11 tagging.
  - "0700" is a POSIX concept and the handshake file carries the auth
    token; Windows needs a user-restricted ACL, now stated and verified
    there.
  - The agent JAR was not wired as a hard prerequisite of the lanes that
    need it, so a stale build would have skipped agent coverage silently.
  - A classification scenario asserted provider behavior the change does
    not contain; trimmed to the classification fact.
  - Mapping-layer unit tests against recorded payloads, so CI covers role
    and attribute mapping without a live JVM.
  - The crate follows the platynui- prefix convention.

  unify-java-provider states its claim rule generally: a window is claimed
  when a backend can serve it. With JAB as the only backend the claimed set
  is unchanged, but the distinction matters — JAB speaks javax.accessibility,
  which only Swing implements, so SWT and JavaFX windows have no JAB
  fallback and stay with the native provider until an agent appears.

- **openspec:** Write out the ten placeholder spec purposes ([2e8681b](https://github.com/imbus/robotframework-PlatynUI/commit/2e8681b22a5332c2ee88eebf8c14fc3bb10d882f))

  Ten of 28 capability specs carried a "TBD - created by syncing/archiving
  change X" placeholder instead of a Purpose.

  Each one is reconstructed from the capability's originating change in the
  archive — the proposal's Why and the design's Context — rewritten into a
  durable present-tense description of what the capability is, then checked
  against the requirement set the spec carries today. A Why argues against a
  gap that existed once, so copying it in verbatim would leave specs
  describing states that no longer hold.

  Two needed more than their originating change, since later changes had
  extended them: baremetal-waiting (add-baremetal-wait-keywords plus
  isolate-baremetal-library-state) and atspi-event-driven-tree (its own
  change plus compositor-popups-and-picker and menuitem-namespace-fix). Both
  later contributions turned out to modify existing requirements rather than
  add any, so the scope is unchanged.

- **openspec:** Archive uia-common-attributes ([8263b56](https://github.com/imbus/robotframework-PlatynUI/commit/8263b569c279d92048e87c44158adf67dac95727))

  Syncs the delta into a new `uia-common-attributes` capability spec and
  moves the change into the archive. All 14 tasks complete; verified on
  Windows with `just check`, `just test` (2072/2072) and the acceptance
  lane at 96/97 — the one failure is the order-sensitive Inspector picker
  test, confirmed against a stashed tree to fail identically on main.

- **openspec:** Record the UIA common-attribute decisions ([3f442d1](https://github.com/imbus/robotframework-PlatynUI/commit/3f442d148a4f6063a03c7c4aae28fef54c2d375a))

  Corrects two design decisions that measurement contradicted during
  implementation, rather than leaving the reasoning that led there:

  D3 assumed top-level windows report keyboard-focusable and that an
  unreadable property should count as "not focusable". Both are wrong. The
  property defaults to FALSE, so the plain accessor conflates denial with
  absence, and an egui window reports true while an Electron one reports
  nothing. The rule and the measurements behind it are now written down, as
  is the rejected Win32 window-style alternative and why it is not needed.

  D7 claimed no live-COM unit lane exists for windows-uia. One does — the
  desktop root is reachable without a fixture app — so the change carries
  tests against real UIA elements. They panic rather than skip when UIA is
  unreachable, which is how they surfaced the cold-start bug fixed in the
  preceding commit; D8 records that.

  architecture.md's Technology example said `UIA`; the registered id is
  `UIAutomation`, which is what the new XPath filters have to match.
  platform-windows.md gains the attributes, the dual-path warning, and the
  focus and editability gates.

- **openspec:** Archive isolate-baremetal-library-state ([8d87739](https://github.com/imbus/robotframework-PlatynUI/commit/8d8773945f3899004acdb0554d40f113c623a94f))

  Applies the change's delta specs to the main specs and moves the change
  into the archive. Verified on both real lanes: Windows 96/95/1 (the one
  failure being the known order-sensitive Inspector picker test) and the
  Linux X11/compositor lanes reported green by the maintainer.

  - baremetal-library-instances: new. Scoped state is private per import
    (variable names derived from the registered library name), the scope
    names are Robot Framework's own with its boundaries, and a root that
    pins an element cannot be given a cross-suite scope.
  - baremetal-selector-resolution: new. A reference carries selector data
    only and is resolved by whoever is running; selectors are re-evaluated
    while captures are pinned, with the scoped root as the documented
    exception; an element belongs to one runtime.
  - baremetal-waiting: modified. "SHALL NOT trust any node cached on the
    shared descriptor" described a mechanism that no longer exists — the
    non-reuse now follows from the general selector rule, and a capture
    from another instance raises instead of being reported as gone.

- **openspec:** Make the agent backend own node validity ([72d9262](https://github.com/imbus/robotframework-PlatynUI/commit/72d92624d63566e590ea849afba10e9bc6de9a54))
- **openspec:** Align provider selection with the write-time root guard ([808e1d3](https://github.com/imbus/robotframework-PlatynUI/commit/808e1d3d1d272fa7eb74e3266b2280c19a80580a))

  The change still argued from the import-args fingerprint, which
  isolate-baremetal-library-state built and then removed in favour of
  guarding the write: a scoped root that pins an element is refused at
  SUITES/GLOBAL, because a suite below builds its own runtime and could
  not resolve that element again. That covers the risk provider selection
  creates — a reader across a suite boundary only ever sees selectors,
  whatever its own selection is — so this change needs no rule of its own.
  The discarded fingerprint is named in the design so a variant of it is
  not reinvented here.

  Also corrected while checking the same area:

  - Each aliased import is its own instance (the library is suite-scoped),
    so identical import arguments are fine and no argument has to be
    varied to force a second session. Measured, and now also the corrected
    reasoning in the coexisting_runtimes suite documentation.
  - Acceptance points at library_instance_isolation.robot as the closer
    precedent: two instances with equal arguments.
  - The mechanism stays the registry; selection is already expressible
    through the library's existing config= parameter, so the shorthand
    parameters are optional sugar rather than part of the surface.

- **openspec:** Record the scope-ladder and root-reuse decisions ([a0232d9](https://github.com/imbus/robotframework-PlatynUI/commit/a0232d9ac0ca918043644dcc39c174804539e5cf))

  Brings the change artefacts in line with what was built, keeping the
  discarded attempts as history so they are not reinvented.

  - D8 now documents all three attempts at the suite boundary and why RF's
    own vocabulary settled it: Set Suite Variable exposes children as a
    trailing pseudo-argument, and the recommended VAR syntax models it as
    the separate scope name SUITES.
  - D3 becomes "guard the write, not the read", holding both discarded
    read-time guards — the import-args fingerprint, whose stated purpose
    did not survive D4/D5, and the owner check on read, which could not
    cover the case that actually breaks.
  - D5a is new: the scoped root is the one place a resolved element may be
    reused, why the runtime side cannot deliver it (each evaluation
    re-reads attributes by design, so keeping XDM wrappers saves the
    enumeration, not the reads), and that the reuse rests on a provider
    implementing is_valid.
  - The Task 1.1 measurements keep their table but gain their validity
    limit: taken on a shallow tree with in-process attribute reads, they
    say nothing about a deep one (Electron, WPF), which is why the
    conclusion holds for target selectors and not for the root.
  - Specs: the scope names and their boundaries, the refusal of a pinned
    root at the cross-suite scopes, and the root-reuse exception with its
    invalidation and cross-import scenarios.

- **openspec:** Reopen the linux verification task ([c405f7e](https://github.com/imbus/robotframework-PlatynUI/commit/c405f7e143ed00fa6ddc3741fee90c7e6b82c8d4))

  Task 7.4 of isolate-baremetal-library-state was checked off while its own
  text recorded that the X11 lane "still needs a Linux host and was not
  run", so the change reported itself complete with a verification
  outstanding. Split it: 7.4 keeps the Windows lane result, 7.5 tracks the
  Linux lanes.

  That gap matters because coexisting_runtimes.robot is platform:x11
  tagged: its two new tests — the per-import root isolation and the live
  foreign-element rejection — never ran on Windows, and the live rejection
  (two runtimes reporting the same provider-stable runtime_id for one
  element, handles still not interchangeable) has no equivalent on the mock
  lane. 7.5 also names the Wayland lane, which 7.4 omitted.

- **openspec:** Archive remove-programmatic-set-text ([5e10266](https://github.com/imbus/robotframework-PlatynUI/commit/5e102662dbdf494dc09f0afad89c897260b69562))

  Applies the change's delta specs to the main specs and moves the change
  into the archive.

  - text-input-policy: new capability recording the cross-provider stance
    (text entry is keyboard-driven; TextEditable is a capability marker).
  - jab-provider: the "Core interaction patterns" requirement drops the
    setTextContents write; its text round-trip scenario becomes the marker
    assertion. This was the last setTextContents mention in the repo — the
    implementation deliberately left it to the sync rather than editing a
    main spec by hand.

- **openspec:** Propose runtime provider selection ([15705c5](https://github.com/imbus/robotframework-PlatynUI/commit/15705c5d46bdb68ae4fbeddd3380e14c7681b51c))

  Which providers a session uses is not selectable in any general way
  today: individual providers grew ad-hoc kill switches, but there is no
  convention, no way to say "this session is Java only", and no surface for
  it in the Robot library or the Inspector.

  Proposed on top of the existing runtime-session-config foundation:

  - providers.<id>.enabled honored for every provider, resolved before
    construction — a disabled provider does no connection attempt and no
    library probing (the Java lane's quiescence, generalized).
  - providers.include / providers.exclude as reserved non-id keys in the
    providers bucket, following the platform.backend precedent, with
    explicit-beats-broad precedence.
  - Mis-selection is loud: an include list matching nothing fails naming
    requested and registered ids; unknown entries alongside valid ones only
    warn, so one config stays portable across operating systems.
  - A construction diagnostic names the rule that suppressed each provider,
    so a filtered provider is not mistaken for a broken one.
  - Robot library import takes the selection, so a suite can load the
    library twice under different names — two independent sessions.
  - Inspector toggles rebuild the runtime instead of mutating a live one,
    preserving the construction-fixed config invariant.

  Needs no dynamic provider loading; that is separate future work.

- **openspec:** Propose unify-java-provider prep refactor ([a98e5a6](https://github.com/imbus/robotframework-PlatynUI/commit/a98e5a6a765872e0bdc8ea8dd9d0b2f9502a2d41))

  Two independent Java claimants (JAB provider + agent provider) would
  have forced window_claims from its boolean "claimed by other" check into
  rank-based ownership — new registry semantics plus updates to every
  generic consumer — purely to coordinate two halves of one thing.

  Consolidating first removes that cost before it is paid: one registered
  provider-java routing per window to toolkit backends, JAB as the first
  backend (behavior unchanged), boolean claims exactly as today. The agent
  then lands as a second backend, and provider-java-swing drops its
  rank-based-claims design entirely.

  Also in scope:

  - Config namespace providers.jab.* -> providers.java.jab.* for all three
    keys (enabled, call_timeout_ms, dll_path — the last one also appears
    in a user-visible error message), plus providers.java.enabled;
    providers.java.agent.* reserved. BREAKING, pre-1.0, no aliases.
  - Classifier consumption moves into provider-java, while detection stays
    in the platform layer: it is the breadcrumb that explains an empty
    Java window and must work when no Java provider is active.
  - Claim-owner id becomes "java" — a consequence of the single-claimant
    decision, tracked separately from the config rename so the literal
    "jab" is not blind-replaced.

- **openspec:** Split java agent proposal per toolkit ([a81d2d4](https://github.com/imbus/robotframework-PlatynUI/commit/a81d2d450426f804466601785dfad19f95fba2bb))

  The single provider-java-agent proposal carried the shared agent
  infrastructure plus three toolkits and read as one oversized change.
  Split it so the urgent part can move on its own:

  - provider-java-swing (renamed from provider-java-agent): the carrier —
    agent artifact, transport, injection, backend routing, plus the
    Swing/AWT adapter, the toolkit with the proven JTable fidelity gap.
  - provider-java-javafx / provider-java-swt: thin adapter changes, each
    depending on the swing change and its fixture change.

  Also decided along the way and folded in:

  - Build and delivery (design 9): agent JAR from a Gradle product
    project, shipped as its own py3-none-any wheel platynui-provider-java,
    exact-pinned [java] extras, opt-in-by-install, discovery via the new
    platynui.providers entry-point group (in-process PyO3 for test runs,
    co-located-interpreter one-shot for the bin wheels).
  - Version mismatch aborts; no compatibility windows.
  - Quiescence: with the agent backend inactive, nothing Java-related runs
    beyond the classifier check; no JVM/JDK/jattach needed on the host.
  - Machine-wide JVM enumeration (jps/hsperfdata) dropped as a non-goal.
  - Mixed-toolkit trees deferred to their own future proposal; the wire
    keeps them cheap (handshake toolkits is a list from day one).

  Names now match the code: crate provider-java, package
  platynui-provider-java, capability java-provider.

- **openspec:** Propose remove-programmatic-set-text change ([da85a0d](https://github.com/imbus/robotframework-PlatynUI/commit/da85a0d14757f9ed9c9e3a434644771dab338547))
- **openspec:** Propose uia-common-attributes change ([c946078](https://github.com/imbus/robotframework-PlatynUI/commit/c946078c7cf93a329034f8a3623c2cbf65cf2252))
- **openspec:** Archive control-description-attribute ([b83f5b1](https://github.com/imbus/robotframework-PlatynUI/commit/b83f5b163c818d1ecc47bb4bafcfc57bdc709526))
- **openspec:** Use last-action observable in swt/javafx proposals ([f831424](https://github.com/imbus/robotframework-PlatynUI/commit/f831424880bf7519de20908bd9c2e70ce78a9c58))

  The catalog requirements, scenarios, and tasks of the open SWT/JavaFX
  fixture changes still specified the rename-on-activate observable
  (<ident>-activated / -clicked). Align them with the blueprint: an
  always-visible last-action-<ident> label reports activations, controls
  never change their own accessible names. Both changes validate.

- **openspec:** Archive tag-based-acceptance-selection ([bf7b174](https://github.com/imbus/robotframework-PlatynUI/commit/bf7b17461d6aa1c4b3ed68c3e1fc823c5cf5fb7f))

  The tag-based lane-selection change is complete: platform tags plus
  real-{x11,wayland,windows} profiles replace runtime Skip If guards, and
  the Windows lane treats the Swing fixture build as a hard prerequisite.
  Verified on all three lanes — on Windows with a fresh full run (93
  tests, 0 skips, Swing 24/24 via the real-windows profile) and the
  fail-not-skip path spot-checked (missing fixture fails all Swing suites
  with the actionable build hint instead of skipping).

  Archiving syncs the deltas into the main specs: the new
  acceptance-lane-selection capability (tags, profile selection,
  fail-not-skip guarantee) and the swing-test-app build requirement's
  skip-to-fail semantics.

- **openspec:** Archive add-qml-test-app ([5996757](https://github.com/imbus/robotframework-PlatynUI/commit/59967574fe1e0aa946effa69076a93f44b61e2e8))

  The QML fixture change is complete and verified on Windows (UIA) and
  Linux (AT-SPI): 23/23 tasks, catalog/modal/custom suites green on both
  lanes, the platform-scoped native-popup suite green on Windows, and the
  last-action observable spot-checked on the real UIA tree.

  Archiving syncs the deltas into the main specs: qml-test-app is created
  (core catalog in QML, dual popup modes, both dialog faces, custom
  chapter, catalog onboarding on both lanes), and test-app-blueprint picks
  up the verified revisions — the extended core tier (group box, text
  area, plain text, image, three-menu bar with submenu), the always-
  visible last-action label replacing rename-on-activate, and the catalog
  suite reframed as a canonical test set implemented self-contained per
  technology instead of shared keyword code.

- **openspec:** Align swt/javafx proposals with canonical catalog test set ([ad7b50c](https://github.com/imbus/robotframework-PlatynUI/commit/ad7b50c0797f259d181dc3f0d3aa09cebc6ba1a5))

  The blueprint no longer ships a shared catalog keyword resource: each
  technology replicates the canonical catalog test set as a self-contained
  catalog.robot, and activations report through the last-action label
  instead of rename-on-activate. Update the open swt/javafx changes'
  proposals, designs, tasks, and spec deltas accordingly.

- **openspec:** Align swt/javafx proposals with tag-based selection ([6cb8dd6](https://github.com/imbus/robotframework-PlatynUI/commit/6cb8dd6c6c4a813dbe6cb850f75ec9ae29910d93))

  The proposals still described the retired soft-skip wiring (mirroring the
  old Swing pattern). Their lane tasks now follow tag-based-acceptance-selection:
  fixture build as a hard lane prerequisite, suites tagged platform:windows,
  and prerequisite checks that fail with the build recipe instead of skipping.

- **openspec:** Align swt/javafx proposals with the fixture blueprint ([f7b9b1c](https://github.com/imbus/robotframework-PlatynUI/commit/f7b9b1c51731cff7c14284ab672f5b4688cab45a))

  Both open fixture changes now implement the test-app-blueprint core-tier
  catalog under the canonical names (incl. group box, text area, plain
  text, image, and the three-menu bar with a nested submenu), adopt the
  blueprint CLI (--open-modal added), and onboard the shared catalog
  acceptance suite via a thin per-technology catalog.robot. The JVM
  classification coverage of both changes is unchanged.

- **openspec:** Archive test-app-blueprint ([1d5c91c](https://github.com/imbus/robotframework-PlatynUI/commit/1d5c91c5dd70e38703be5ec965eb3f19cb275e3a))

  Add the fixture blueprint as a capability: a tiered control catalog under
  canonical names, name-based action observables, a common fixture CLI, and
  shared catalog acceptance-suite conventions that every UI-technology
  fixture app implements. testing-strategy.md §5 is rewritten from the
  two-tier description to the fixture technology matrix, with the
  developer-facing blueprint in the new §5.1; the capability spec is the
  acceptance-criteria form of the same contract and is synced to
  openspec/specs as part of archiving the change.

- **openspec:** Align swt/javafx proposals with the Gradle pattern ([b24c018](https://github.com/imbus/robotframework-PlatynUI/commit/b24c018d53a9c80ef19bb48b44e28516dc52464b))

  The archived migrate-swing-test-app-to-gradle change settled the Java
  fixture toolchain: current Gradle with the wrapper client on the PATH
  java 8+, daemon JVM via committed daemon JVM criteria, Foojay resolver
  convention 1.0.0, and writeJavaLaunchers publishing the provisioned
  launcher paths. Fold that into add-swt-test-app and add-javafx-test-app
  (drop the obsolete Gradle 8.x pin and the outdated "Swing stays javac"
  framing; reuse the Swing scaffold instead of re-deciding it).

- **openspec:** Archive migrate-swing-test-app-to-gradle ([48973d7](https://github.com/imbus/robotframework-PlatynUI/commit/48973d77bf2fde6f1ee143938fe1e8b444ded414))

  Sync the delta spec into specs/swing-test-app (javac-only requirement
  replaced by the self-bootstrapping Gradle wrapper build and an explicit
  dual-runtime requirement) and move the completed change to the archive.

- **openspec:** Archive inspector-window-controls ([8a5f753](https://github.com/imbus/robotframework-PlatynUI/commit/8a5f753da7477641ee5d60d82e34b092af294f14))
- **openspec:** Propose inspector-window-controls ([b72f2bd](https://github.com/imbus/robotframework-PlatynUI/commit/b72f2bd730c4f54de34e805d2e71ed402af956ce))
- **openspec:** Archive inspector-theme ([cf81666](https://github.com/imbus/robotframework-PlatynUI/commit/cf81666b86a9a58337973c12ba83a212d611d31e))
- **openspec:** Archive inspector-toolbar-redesign ([a8adee1](https://github.com/imbus/robotframework-PlatynUI/commit/a8adee1949eab6dba35f98f771c102112db6ecbe))
- **openspec:** Propose SWT/JavaFX fixtures + Swing Gradle migration ([060b8dc](https://github.com/imbus/robotframework-PlatynUI/commit/060b8dc382b3958f9e9ddb4793ec062c9873fa25))

  Three fixture changes sharing one toolchain policy: checked-in Gradle
  wrapper (8.x, runs on the PATH java 8+), JDK auto-provisioning via the
  Foojay resolver, libraries from Maven Central.

  - add-swt-test-app: SWT fixture, compiled --release 8 against the last
    Java-8-compatible SWT, tested on a provisioned JRE 8 and on 21
  - add-javafx-test-app: JavaFX fixture via org.openjfx.javafxplugin,
    21-only (OpenJFX has no Java 8 artifacts on Maven Central)
  - migrate-swing-test-app-to-gradle: retire the javac-only build in
    favor of the shared pattern, launch contract preserved

- **openspec:** Archive java-app-classifier and sync specs ([37070b0](https://github.com/imbus/robotframework-PlatynUI/commit/37070b07c8a943583ded9f91686f786befec31f8))

  All 12 tasks complete and verified (check/test/build-native + Windows
  acceptance). The java-app-classification delta becomes a main spec.

- **openspec:** Propose staged Java detection + agent provider ([976be3c](https://github.com/imbus/robotframework-PlatynUI/commit/976be3c7cab38373be432d7fc664b434defc0cba))

  Two staged changes for Java UI automation beyond JAB:

  - java-app-classifier (apply-ready): a provider-independent, cross-
    platform classifier that identifies JVM-backed windows and their
    toolkit and surfaces it as observable facts plus a generalized
    "accessibility not reachable" diagnostic. Detection only — no
    injection, no consent, no ownership change.

  - provider-java-agent (spike-gated): an in-JVM agent-backed provider.
    Design decided across transport (control/data split, loopback TCP +
    NDJSON-RPC, handshake file for discovery/auth), data model (instance
    spine + accessibility enrichment + virtual subtrees), rank-based
    window_claims, native Rust attach transport, multi-client threading,
    physical-pixel coordinates, and v1 scope = Swing/AWT only. Remaining
    items are spike verifications, not open decisions.

- **openspec:** Propose inspector-toolbar-redesign ([23ae1fc](https://github.com/imbus/robotframework-PlatynUI/commit/23ae1fc7e62ba58207408fceaf79fc7d6cf4b46b))

  Toolbar panel with themed background (fixes the unthemed picker strip),
  slimmed search row, segmented status bar with a persistent picker-state
  segment, embedded Phosphor SVG icons with a configurable IconsOnly/
  IconsAndText style, and stable AccessKit author_id element IDs surfacing
  as the common @Id attribute for acceptance-test locators.

- **openspec:** Propose fix-jab-hit-test-virtual-children ([b4bf2b2](https://github.com/imbus/robotframework-PlatynUI/commit/b4bf2b2eadb7cafd6567ac1f4fce79a1141232bf))

  The Inspector picker cannot reveal JTable cells, headers, or even the
  table: with the pointer hovering, the native hit-test returns virtual
  wrappers (AccessibleJTableCell / AccessibleJTableHeaderEntry) that
  isSameObject can never match against enumeration children, so the
  reveal degrades to a parentless fallback node. Proposal: per-level
  indexInParent fallback (range-guarded) plus deepest-matched-ancestor
  degradation instead of the parentless node.

- **openspec:** Archive add-jab-interface-attributes and sync specs ([8f754d0](https://github.com/imbus/robotframework-PlatynUI/commit/8f754d014ddcdb4390fa542a9fea7b7582026383))

  All 14 tasks complete and verified (just check, 2016/2016 tests, live
  JAB tests, Swing lane 24/24, full Windows acceptance at the known
  baseline). Delta specs synced to the new main spec
  openspec/specs/jab-interface-attributes/spec.md, including the
  amendments discovered during implementation (renderer aliasing,
  enumeration-listed per-cell attributes with lazy values).

- **openspec:** Archive add-jab-hit-test ([caa8b4d](https://github.com/imbus/robotframework-PlatynUI/commit/caa8b4da6c8d9a41ae3a58ced41ef0bc96238037))

  Implementation complete (f011705) and the jab-hit-test delta spec is
  synced into openspec/specs; move the change to the archive.

- **openspec:** Propose add-jab-interface-attributes ([c7041cb](https://github.com/imbus/robotframework-PlatynUI/commit/c7041cbee15745fc78dd3af51b6cd2cef7563c90))

  Propose projecting the data of each supported JAB accessibility
  interface (table, value, text, action, hypertext, key-bindings,
  relations) onto UiNode as native:* attributes, mirroring the UIA
  provider's collect_native_properties. Bitfield-gated, live reads,
  lazy per-cell resolution; adds a JTable stage to the Swing fixture.

- **openspec:** Propose add-jab-hit-test ([b11b427](https://github.com/imbus/robotframework-PlatynUI/commit/b11b427d71169ee7691630b72354537d1ecafd3b))

  Propose point-based hit-testing for Java Swing/AWT windows via the
  JAB provider (Inspector picker / element_at_point), with UIA
  abstaining on claimed windows for order-independent arbitration.

- **openspec:** Archive add-jab-provider and sync specs ([f2a4950](https://github.com/imbus/robotframework-PlatynUI/commit/f2a49507d62f26617d4622af2b02e550ae55c5a8))

  Move the completed add-jab-provider change to the archive and
  promote its delta spec into the main specs as the new
  jab-provider capability (12 requirements).

- **openspec:** Sync add-jab-provider artifacts with the implementation ([8bf9fc6](https://github.com/imbus/robotframework-PlatynUI/commit/8bf9fc6af23b7252740e26061f4b6e7a280ea165))

  Fill the design Findings, resolve the DPI and JList open questions,
  record the live-read model (correcting the earlier events false alarm),
  mark all tasks done, and align the specs with what shipped.

- **openspec:** Propose control-description-attribute change ([60e694c](https://github.com/imbus/robotframework-PlatynUI/commit/60e694c396e802ec99b716b3e036430ed9b3dd49))

  Introduce control:Description as a common attribute sourced strictly
  from the accessible description (UIA FullDescription, AT-SPI
  Accessible.Description; no HelpText fallback), emitted only when
  non-empty, with a first-class UiNode/Python accessor, mock support,
  architecture.md updates, and an egui/AccessKit acceptance test.

- **openspec:** Propose add-jab-provider ([cc6e5af](https://github.com/imbus/robotframework-PlatynUI/commit/cc6e5af418a902bb10b1ec7febd57b6e7f718361))

  Proposal, design, spec, and tasks for the Windows Java Access Bridge
  provider (variant B: a native provider crate mirroring provider-atspi;
  the UIA client-side proxy variant is parked in the design with
  rationale). The design records the research (threading model, handle
  lifecycle, DLL discovery, security constraints) and the spike findings
  from add-swing-test-app, including the harvested role vocabulary and
  the indexInParent reliability caveat.

- **openspec:** Archive add-swing-test-app and sync specs ([971b4d1](https://github.com/imbus/robotframework-PlatynUI/commit/971b4d147229bc33e53c03aa34958e3ee0db40f3))
- **openspec:** Archive menuitem-namespace-fix and sync specs ([dbcef73](https://github.com/imbus/robotframework-PlatynUI/commit/dbcef7341224403d06e891611ba5ac1090e0a636))

  Move the completed change to the archive (18/18 tasks) and sync its
  deltas into the main specs: new menu-item-namespace capability pinning
  menu-entry roles to the control namespace across providers, and the
  atspi-event-driven-tree popup scenario's example locator updated from
  //item:MenuItem to //MenuItem.

- **openspec:** Archive inspector-live-mouse-picker and sync specs ([8696382](https://github.com/imbus/robotframework-PlatynUI/commit/869638288d2777fdf828da96b918effdf1471f77))

  Sync the element-at-point and inspector-live-mouse-picker delta specs
  into the main specs and archive the completed change (34/34 tasks; both
  acceptance lanes green).

- **openspec:** Archive compositor-popups-and-picker and sync specs ([ce4302f](https://github.com/imbus/robotframework-PlatynUI/commit/ce4302ff19c6b03473abcd2ba3da814582fda5a0))

  Delta specs merged into main specs: atspi-event-driven-tree's cascade
  scenario loses its Wayland caveat; new capabilities
  compositor-popup-geometry and compositor-modifier-state.

- **openspec:** Add compositor-popups-and-picker change proposal ([196f301](https://github.com/imbus/robotframework-PlatynUI/commit/196f3011efc36c07bf3903308518ebfd69e46917))
- **openspec:** Archive atspi-event-driven-tree and sync spec ([58d3f48](https://github.com/imbus/robotframework-PlatynUI/commit/58d3f48986fc4facc422efb271eaf97396067d2b))
- **openspec:** Add atspi-event-driven-tree change ([a476060](https://github.com/imbus/robotframework-PlatynUI/commit/a4760603e89ebaeefabfaa150b3562ab6f8c977a))

  Propose making the AT-SPI provider event-aware so transient popups (context
  menus, dropdowns, tooltips) appear in the tree and become findable by XPath, the
  Inspector, and the hit-test/picker. Documents the verified Qt behavior: the
  popup is exposed only event-driven and asymmetrically (a PopupMenu child of the
  Application reachable via parent() and getChildren down from the popup, but
  absent from the Application's own getChildren), so a top-down walk misses it. The
  design subscribes to structural events on a dedicated connection and grafts the
  popup under its owner. Unblocks the deferred context-menu limitation in
  inspector-live-mouse-picker.

- **openspec:** Add MenuItem namespace fix change proposal ([89c273e](https://github.com/imbus/robotframework-PlatynUI/commit/89c273e5d5685afd987b8480790a941b50dfc080))
- **openspec:** Archive add-textcontent-pattern and sync spec ([08ca391](https://github.com/imbus/robotframework-PlatynUI/commit/08ca3910d257d65ae469c7bb339f481ddd161763))

  Move the completed change to archive/2026-07-07-add-textcontent-pattern
  and promote its delta spec to openspec/specs/textcontent-pattern.

- **openspec:** Archive add-baremetal-pointer-scroll and sync spec ([4e3e104](https://github.com/imbus/robotframework-PlatynUI/commit/4e3e104143a4ecfa9bf90b4f9a89b1567f021649))

  Move the completed change to archive/2026-07-07-add-baremetal-pointer-scroll
  and promote its delta spec to openspec/specs/baremetal-pointer-scroll.

- **openspec:** Add per-runtime platform sessions change proposal ([63e61e5](https://github.com/imbus/robotframework-PlatynUI/commit/63e61e59c59a1ccd9249042a62a79ff8581af0a7))

  Rework the platform layer from process-global singletons to per-runtime,
  session-bound instances, and add a construction-time `config` dict binding
  each runtime to a specific session. Fixes the X11 multi-suite acceptance
  failure (global OnceLock connection that can't rebuild after shutdown) and
  the identical X11/Windows highlight bugs by construction.

  Phase 1 scoped to Linux X11; Wayland globals internalization, concurrent
  sessions, PlatynUI scope=SUITE, and Windows/macOS parity are follow-ups.
  Supersedes the narrow X11-reconnect fix.

- **openspec:** Add inspector XPath history change proposal ([6a4d0da](https://github.com/imbus/robotframework-PlatynUI/commit/6a4d0da6fabe707218f4ef0dccfdb6d5a9e742ce))

  Persisted, de-duplicated XPath search history (max 100 entries) for the
  Inspector, with a history dropdown and history-sourced as-you-type
  completion. Records only successfully evaluated expressions; selecting a
  history entry only fills the field.

  Adds proposal, design, specs (inspector-xpath-history,
  inspector-xpath-completion), and tasks.

- **openspec:** Add TextContent pattern change proposal ([96c48de](https://github.com/imbus/robotframework-PlatynUI/commit/96c48de47fd6bb4cd46579620b9b34a1866d53a0))

  Planning artifacts for TextContent, the read-only text ClientPattern:
  control:Text sourced only from a genuine text interface (AT-SPI Text;
  UIA TextPattern then ValuePattern), no name fallback and no IsReadOnly.
  Surfaced via Python attribute-only synthesis; atspi-first, with Windows
  implemented and runtime-verified on a Windows host.

- **openspec:** Tailor config context and per-artifact rules ([ccfbda0](https://github.com/imbus/robotframework-PlatynUI/commit/ccfbda0adb620527fc89a55886ef83de5a04ea7c))

  Replace the bare config.yaml template with a high-signal `context` that points
  to the source-of-truth files (AGENTS.md, CONTRIBUTING.md, README, dev-docs/)
  instead of duplicating them, plus concise, general per-artifact rules
  (proposal/specs/design/tasks) layered on top of the spec-driven schema.

- **plan:** Update section headers and reorganize task lists in implementation plan ([88ec022](https://github.com/imbus/robotframework-PlatynUI/commit/88ec022c144f9420728b26d4af36a7980eed3167))
- **python:** Rewrite Get Element At Point docstring in user-facing style ([f8b1e80](https://github.com/imbus/robotframework-PlatynUI/commit/f8b1e80d5fe6aca6b65d45750042166a23c6b7cb))
- **python:** Tighten docstrings and rename 'page object' to 'context' ([32f35a0](https://github.com/imbus/robotframework-PlatynUI/commit/32f35a0ce206e5bd488e69d7d3aaa8a6e537c645))

  Sweep across src/PlatynUI, tests, and design docs. Removes
  implementation/capability detail from module and class docstrings
  (no longer enumerates Properties or pattern names already visible
  in the signature) and unifies terminology with ContextBase / @context
  by replacing the overloaded 'page object' wording with 'context' /
  'Context-Klasse'.

- **readme:** Refresh tooling docs and platform-support table ([c4074e8](https://github.com/imbus/robotframework-PlatynUI/commit/c4074e85fb1a23c09197c4d8e3e40699a5d76e0d))

  Document the newer CLI subcommands (highlight, screenshot,
  element-at-point, watch, focus, window, pointer) and the Inspector's
  live mouse picker, theme following, and settings persistence.

  Rework the platform-support table with a dedicated PlatynUI compositor
  column so generic Wayland status is no longer conflated with what only
  works under the project's own compositor. Correct the Java row: per
  dev-docs/java-toolkits.md, java-atk-wrapper is deliberately not part of
  the Linux strategy — Swing/AWT there is planned as an in-JVM agent.

- **readme:** Refresh project overview and package docs ([fa6a0fe](https://github.com/imbus/robotframework-PlatynUI/commit/fa6a0fe914946c1c183812ecc8d48b86c3bdbcbb))
- **runtime:** Describe the XPath cache as the snapshot it is ([18353c3](https://github.com/imbus/robotframework-PlatynUI/commit/18353c3a67e305c14f52095a943e4e5895289766))

  The docs said the runtime creates no cache and refreshes the tree of a
  provider without events before each query. Neither holds: every Python
  evaluation goes through a cache the runtime owns, and only its caller
  clears it. They now explain the snapshot model as designed, and mark the
  event table and the planning checkboxes that no longer match the code.

- **runtime:** Describe scroll_into_view action for control and item elements ([2bf4450](https://github.com/imbus/robotframework-PlatynUI/commit/2bf44504c886163ccc781985f95e9e8c92442b4f))
- **runtime:** Rename implementation plan for PlatynUI Runtime ([4dd42dd](https://github.com/imbus/robotframework-PlatynUI/commit/4dd42dd9c3fd4100af981c1603ae74e7cbad6612))
- **runtime:** Clarify control vs app namespace usage ([5746ce1](https://github.com/imbus/robotframework-PlatynUI/commit/5746ce10fd7b5594d52347539aac4bceffc082d5))
- **runtime:** Clarify WindowSurface guidance ([17ab132](https://github.com/imbus/robotframework-PlatynUI/commit/17ab1320fcc9dc4a1973a3c396a33aa2d4ed7488))
- **test-app-gtk:** Describe how PlatynUI places GTK elements ([fe40011](https://github.com/imbus/robotframework-PlatynUI/commit/fe4001199057352b985da671efd8c7ab4c17a9da))

  The fixture READMEs still said the pointer and popover tests stayed out
  of the suites until linux-element-geometry landed. Describe the
  placement instead and point to dev-docs/platform-linux.md.

- **testing:** Get Attribute checks once and does not wait ([c3d4055](https://github.com/imbus/robotframework-PlatynUI/commit/c3d4055e69de6f3d66e3dfe0816bb1387e2e6030))

  The robot-test-style skill and testing-strategy.md said `Get Attribute
  ... == ...` retries until the assertion passes. It does not: `@assertable`
  runs the keyword once and verifies once; only resolving the element
  waits. On the synchronous mock that goes unnoticed, but in the real lanes
  window-manager effects land later and the one-shot check fails.

  Both now point to `Wait Until Query .../@Attr == ...` for a value that
  still has to change, keep `Get Attribute` for a check of the value as it
  is now, and explain how to assert that a state does not change.

- **windows:** Note dpi-aware pointer task ([7b1976e](https://github.com/imbus/robotframework-PlatynUI/commit/7b1976e2e854fda73e4c753cb9bc27b3b1e6fa1d))
- **xpath:** Update XPath 2.0 coverage notes for compatibility mode and namespace tracking ([b80aef7](https://github.com/imbus/robotframework-PlatynUI/commit/b80aef77981d9df44c6050ecbd37b76d1ea189d8))
- **xpath:** Add a Readme and comprehensive coverage documentation for XPath 2.0 compliance ([3a0920e](https://github.com/imbus/robotframework-PlatynUI/commit/3a0920eb99387fa8ccaa41b8042ee35840ffd1f2))
- Name the sources of Name and Description per provider ([b1f202e](https://github.com/imbus/robotframework-PlatynUI/commit/b1f202e7ebb10e255c15f1b28b9204623ea44502))


  The architecture named no source for Name beyond UIA and AT-SPI and no
  Java source for Description. It now states that Name is the accessible
  name with no fallback on AT-SPI and both Java backends, where the values
  that are not the name stay readable, and the Description sources of the
  bridge and the agent, including the one Swing derives from a tool tip.
- Explain document order and per-step positions in XPath ([2141f21](https://github.com/imbus/robotframework-PlatynUI/commit/2141f215b2d95b10ef6c623a8fbe3d03913b5cd2))


  Describe how the engine keeps results in document order and where it
  sorts, how positional predicates count per step, the rewrites into one
  descendant step, and why a first match stays cheap. Add the Java
  agent's per-view runtime ids to the RuntimeId schema, the Locator's
  descendant:: rendering to the library design, and to BareMetal's
  Finding elements (//X)[n] against //X[n], with the (.//X)[1]/*[n]
  idiom for a container known to be unique.
- State that attribute values are passed through as reported ([cfb9e64](https://github.com/imbus/robotframework-PlatynUI/commit/cfb9e6479376d0f1165ca4efdf7f1772aad9c1f3))


  The architecture builds on typed values that mirror the platform, but
  only the Description said "unmodified". Section 5.7 now states the rule
  for every provider and attribute group, 5.5, 5.6 and 6.3 say how Id,
  Description and Name treat the empty string, and 7.3 makes it a
  provider checklist item.
- Name Get Attribute Value in the testing guidance ([eebc7ef](https://github.com/imbus/robotframework-PlatynUI/commit/eebc7efaa7303860263e9d83c8badfafc4d7a51e))
- Keep the README for users and give contributors the rest ([0dae9fb](https://github.com/imbus/robotframework-PlatynUI/commit/0dae9fb83fe87fd9edcc15ce27e1179fca7387a7))


  The README had grown into developer notes (crate names, OS APIs,
  dev-docs links, build steps), yet it never showed how to install the
  library itself. It is also the package's PyPI page, where its
  relative links led nowhere.

  What contributors need lives in CONTRIBUTING.md and the architecture
  doc instead: the backend support matrix, the OpenSpec workflow, and
  the commit and test rules that so far lived only in review feedback.
- Describe the process attributes of an application ([4a07527](https://github.com/imbus/robotframework-PlatynUI/commit/4a0752777aad3f17a12a0ebe3e99910ca12fa39c))


  Users read these attributes in selectors, with Get Attribute and in the
  Inspector, but no user documentation described them, and the design docs
  still named the old per-provider sources: comm, the executable stem, the
  PE header. BareMetal gains a "Process attributes" section, and the dev
  docs name the shared reader and point to the spec for the formats.
- Say when PlatynUI reads the UI again and when roots move ([aebf956](https://github.com/imbus/robotframework-PlatynUI/commit/aebf956feb69a04a606acf2d0d94e98379e7f13c))


  The BareMetal introduction promised that every query sees the live
  screen, never a stored snapshot, which is not how the library works:
  keywords reuse a snapshot while its elements exist and read the screen
  again after a miss, before Query and in the waits. Users need that to
  know why a changed UI must be waited for.

  The developer docs record that application nodes now follow their
  process, that PlatynUI.core discards the snapshot as well, and when a
  pinned Java application root moves between the agent and the bridge.
- Describe snapshot lifetimes and the provider parent rule ([01517c6](https://github.com/imbus/robotframework-PlatynUI/commit/01517c6d0403d7820e8f674e4f6a32149f6a1ca2))


  Developers need to know when a snapshot's memory is returned, that a
  held node keeps only its own ancestors, and what a provider owes the
  runtime for that: the parent of every listed node kept alive, no node
  holding its children. Event-driven invalidation is marked as not
  pursued, by the maintainer's decision: it costs more than it saves.
- Describe the agent backend and the dev-loop traps around it ([0489aa9](https://github.com/imbus/robotframework-PlatynUI/commit/0489aa9c84b7a0e264cb7c59cf905e99e1e2b112))


  Rewrites the Java section of the Windows platform doc around two
  backends instead of one, and records the facts that are easy to get
  wrong from reading the code alone: that selection *is* the backend
  order, that a timed-out connection has to be rebuilt rather than
  retried, and that `@Technology` names the channel while the toolkit
  lives in `native:JvmToolkit`.

  Two contributor-facing traps get written down because both cost real
  debugging time in practice:

  - Installing the provider package is a deliberate, separate step, so a
    source build serves Java through the Access Bridge and looks
    unchanged.
  - A rebuilt JAR is not a delivered JAR: `just build-java-agent` writes
    build/libs, but the installed package keeps serving the staged copy
    until `just build-provider-java` restages it — and the provider↔agent
    version handshake cannot notice, because both sides still report the
    same dev version.
- Note provider-plugin ABI constraints and option space ([a796bc7](https://github.com/imbus/robotframework-PlatynUI/commit/a796bc79964e8de2dff335cd1c7f20a3d916db8b))


  Captured while designing the Java agent provider so the constraints do
  not have to be re-derived when the plugin proposal is written. Notes
  only — no decision is made here.

  Records what "same compiler" actually requires for Rust cdylib plugins
  (rustc patch version, codegen flags, identical interface-crate graph
  including feature unification, allocator/std coupling and duplicated
  globals — which is how providers register today), why a violation is UB
  rather than an error, what abi_stable mechanizes and at what cost, and
  the four-option space including out-of-process RPC. Two observations
  worth keeping: a WPF provider can never be a Rust-ABI plugin, so any
  cross-language ambition lands in C-ABI or RPC regardless; and the Java
  agent already is the RPC shape, so distilling the protocol from the
  hardened wire beats designing it speculatively.
- Add robot-test-style skill routed from AGENTS.md ([89ea858](https://github.com/imbus/robotframework-PlatynUI/commit/89ea858eaf5adf88af527b5e4bffe4c0ee07f9c0))


  Capture the Robot Framework suite-authoring conventions as an on-demand
  agent skill (.claude/skills/robot-test-style): suite anatomy, inline
  relative locators, the self-waiting keyword table, observables, RF >= 7
  idioms, the three-dimensional tag model, and lane verification recipes.
  dev-docs/testing-strategy.md stays the normative source; AGENTS.md gains
  a routing line.
- Add cross-platform Java-toolkit detection/coverage reference ([2d8ebcc](https://github.com/imbus/robotframework-PlatynUI/commit/2d8ebcce8191a2332684690e31661dc83a12ab51))


  New dev-docs/java-toolkits.md maps how PlatynUI recognizes JVM-backed
  windows and which accessibility path reaches each toolkit per platform:
  the is-JVM primitive (jvm.dll / libjvm.so / attach artifacts), toolkit
  discrimination (window class on Windows, fuzzier elsewhere), the native
  coverage matrix, the decision not to rely on java-atk-wrapper on Linux,
  and the JavaFX-on-Linux gap (no native a11y at all). Also records the
  agent injection paths and the JEP 451 sunset for dynamic attach.

  Fixes three defects in platform-windows.md's JAB section: a mis-merged
  interface-attribute paragraph, a stale "Linux lane unaffected" line, and
  the outdated never-instrument framing around the agent option. Indexes
  the new doc in AGENTS.md.
- Document the Java Access Bridge provider ([e84aa12](https://github.com/imbus/robotframework-PlatynUI/commit/e84aa121adafc039d8a12fcd71ef1c0259d1359e))


  Add a JAB provider section to dev-docs/platform-windows.md (threading
  model, handle discipline, live-read model, DPI calibration, claims,
  diagnostics, config keys), a crate-landscape entry and RuntimeId table
  row to dev-docs/architecture.md, the crate to the AGENTS.md list, and a
  Java Swing/AWT row plus provider bullet to the README platform-support
  table (experimental on Windows via JAB; Linux via AT-SPI2).
- Correct AGENTS.md — packages/native is a Cargo workspace member ([7393e55](https://github.com/imbus/robotframework-PlatynUI/commit/7393e55647e41d34edb10617e8fc0c5108f70adc))


  The root Cargo.toml includes `packages/*` in `members` (the exclude line is
  commented out), and `cargo metadata` lists platynui_native among the workspace
  members, so the workspace-wide `just check`/`just test` gates cover it. The doc
  had claimed it lives outside the workspace; only the package *name*
  (platynui_native) follows Python conventions.
- Reflect per-runtime platform sessions in dev-docs ([c07f35b](https://github.com/imbus/robotframework-PlatynUI/commit/c07f35bb4f788a6e58a9eab6d5db7920944526d4))


  Update the developer docs to match the per-runtime platform architecture
  (change per-runtime-platform-sessions §8):

  - architecture.md: registration model (two factory macros), the Linux mediator
    (two PlatformFactories, no RESOLVED/wrappers), init/shutdown lifecycle
    (per-runtime bundle, no reference-counted lease), desktop-node source, and
    test injection (mock via config backend).
  - platform-linux.md: design decisions + selection example rewritten to the
    factory model; owned Arc<X11Connection> and per-instance highlight thread.
  - python-bindings.md: new Runtime Configuration section (Runtime(config) dict
    shape, leaf mapping, tolerant unknown keys, construction-time immutability).
  - testing-strategy.md: normative note that the multi-suite acceptance lanes
    guard per-runtime isolation.

  python-library-design.md (German migration draft) is deferred -- config= is
  covered by the BareMetal docstring, python-bindings.md, and architecture.md.
- Require test-first RF keyword tests and GWT acceptance style ([8bbe343](https://github.com/imbus/robotframework-PlatynUI/commit/8bbe3436395781fbd57d170aaa465713d3d019be))


  Establish test-first for changes whose target is the Robot Framework
  library, and align the OpenSpec config with the testing strategy that
  now documents it.
- Move developer docs to dev-docs/, reserve docs/ for user docs ([6ea8bdb](https://github.com/imbus/robotframework-PlatynUI/commit/6ea8bdb64c51ad321caa9ee8d59ae3d46b2492cc))


  Developer, design, and planning documentation moves from docs/ to a new
  top-level dev-docs/ tree. docs/ now holds only a README pointer and talks/,
  and is reserved for user-facing documentation.

  - Rewrite testing-strategy.md as a normative strategy (what/how/where and how
    tests are built), dropping the stale status-report content.
  - Add docs/README.md (pointer) and dev-docs/README.md (index).
  - Update all references to the moved docs: AGENTS.md, CONTRIBUTING.md, the root
    and package/app/crate READMEs, code comments, and the pyproject ruff globs.
- Restyle AGENTS.md to repository-guidelines format ([131e404](https://github.com/imbus/robotframework-PlatynUI/commit/131e4041188034189721382d6f4ea04e9ab2b65c))


  Adopt a structured layout (Quick Orientation, Task Routing, Common Commands, Design Docs, Commits, Agent Notes) and correct the language note: the design docs are English; only some planning docs under docs/ are German.
- Sync design docs with current implementation ([34f195a](https://github.com/imbus/robotframework-PlatynUI/commit/34f195a305e69dd78b034216b188519f361920aa))


  Reconcile the design docs with the code (code is the source of truth):

  - architecture: decomposed window-capability patterns, XdmNode GATs and
    method/return changes, UiNode/UiAttribute and WindowManager signatures,
    test-fixture locations, Linux mediator/Wayland status
  - error-handling: document PatternError plus KeyboardError/KeyCodeError,
    the AtspiError/UiaError From asymmetry, and the Python FFI error mapping
  - platform-linux/windows/wayland: ZST-delegation mechanism, AT-SPI 0.14,
    BGRA8 screenshots, RuntimeId format, ProcessName attribute, phase status,
    remove the unimplemented AccessKit claim, add eis-test-client
  - inspector: view/viewmodel module trees, reveal_node_cached, TreeView API
  - keyboard-input: generic escape, whitespace separator, LMeta/RMeta
  - pointer-input: clarify ms (CLI) vs us (Python) timing units
  - python-bindings: XDM cache type, decomposed patterns, exception order
  - python migration-status/library-design: committed-status and line/LOC fixes
  - testing-strategy: test-app-egui path, eframe 0.34, attribute and keyword
    counts, ava.robot
- Align design and status docs with new container contexts ([f876070](https://github.com/imbus/robotframework-PlatynUI/commit/f876070ecff49fe37840c159d85247c1a41c8127))


  Modernise the List/Tree/Table/ComboBox code sketches and prose to
  the keyword-only locator/scope API and mirror the iter_items
  generator fix. Tick off all phase 4c checkboxes in the migration
  status.
- Document Text/Edit page-objects and flat pattern hierarchy ([4d3ee1f](https://github.com/imbus/robotframework-PlatynUI/commit/4d3ee1fdd1f3c8e3c028da28d5945bd4a87d714a))


  Adds design sections A.14.10 (Text) and A.14.11 (Edit) covering
  the read-only and editable text widgets, their pattern
  dependencies, predicate verification, and the rationale for
  keeping them as independent classes without a shared base.

  Records the multi-line decision: is_multi_line lives on
  TextEditable, not TextContent, because the property has no
  behaviour effect on read-only displays.

  Adds a clarifying paragraph and changelog entry stating that the
  pattern hierarchy stays flat across all capability pairs, since
  the pattern_name-based resolver ignores Python inheritance and
  adapters must list each Reverse-DNS name explicitly anyway.
- Reorder phase 4 sub-phases and document buttons design ([2774120](https://github.com/imbus/robotframework-PlatynUI/commit/277412012829c8eaf8264c58bc21a5f80a791136))


  UI page-objects are now implemented before the default-proxy layer
  so each widget gets exercised against the provider-pattern path
  first; proxy fallbacks are bundled into the final sub-phase.

  Adds the AbstractButton/Button/CheckBox specification and updates
  the migration status accordingly.
- Update project structure and planning documentation with new app and crate details ([0d4a5c6](https://github.com/imbus/robotframework-PlatynUI/commit/0d4a5c694ae6cea1707bb8161cdebbc004d15fb7))
- Add accessibility guide and testing strategy documentation ([b7f7b35](https://github.com/imbus/robotframework-PlatynUI/commit/b7f7b3595ac9d914afc5136d2f2b071296cf92c2))


  - Introduced `egui-accessibility-guide.md` detailing the integration of AccessKit with egui, including available widgets, their roles, and accessibility API levels.
  - Added `testing-strategy.md` outlining the current state of testing, identified gaps, and a proposed test layer model for improving coverage and integration across platforms.
- Update feasibility study for platform-linux-wayland crate with recent findings and protocol updates ([951d900](https://github.com/imbus/robotframework-PlatynUI/commit/951d9005e5ac9a39e559d17331375f34b9819be5))
- Add feasibility study for platform-linux-wayland crate ([f4d95c9](https://github.com/imbus/robotframework-PlatynUI/commit/f4d95c9a1e954c7596e72d36812376eaed9529b9))
- Update architecture, CLI, planning, and Python bindings documentation for clarity and completeness ([1d4fe5a](https://github.com/imbus/robotframework-PlatynUI/commit/1d4fe5a3077903f7b2df767e57da8aad93a46b7c))
- Consolidation of architekture and planning documents ([2116212](https://github.com/imbus/robotframework-PlatynUI/commit/21162125fecdf3f07304e428ae5d3a2a16f6cc96))
- Update some implementation docs ([e8da1e6](https://github.com/imbus/robotframework-PlatynUI/commit/e8da1e652e75354453f0e77ab130f699839704c9))
- Add virtual desktop handling design and integrate into existing docs ([c662c0f](https://github.com/imbus/robotframework-PlatynUI/commit/c662c0f5be8d712b35d490f5ca7fc52292350dac))


  Introduce `docs/virtual_desktop_switching.md` with a pragmatic
  `ensure_window_accessible(WindowId)` approach instead of a full
  cross-platform desktop API. Each platform solves internally:
  - X11: switch desktop via `_NET_CURRENT_DESKTOP` EWMH ClientMessage
  - Windows: move window via `IVirtualDesktopManager::MoveWindowToDesktop`
  - macOS: no-op (kAXRaiseAction handles Space switch implicitly)

  Integrate the design into existing documentation:
  - architekturkonzept_runtime: extend WindowManager trait definition,
    EWMH-Support check, and add virtual desktop paragraph
  - architekturkonzept_runtime_umsetzungsplan: add implementation tasks
    to Phase 1 (trait), Phase 2 (X11/EWMH), Phase 3 (Windows COM),
    and CLI bring_to_front integration
  - linux_x11_implementation_plan: add desktop atom tasks to Phase 3
  - patterns: note that providers are not responsible for desktop switching
  - provider_checklist: clarify Runtime/WindowManager responsibility
  - provider_windows_uia_design: reference WindowManager in outlook
- Add initial presentation for RoboCon Talk on PlatynUI ([9a9d665](https://github.com/imbus/robotframework-PlatynUI/commit/9a9d665e423af7e0b478a76e376cab593817cca1))
- Update Windows ApplicationNode id() fallback to include optional AUMID as a stable identifier ([0ace67e](https://github.com/imbus/robotframework-PlatynUI/commit/0ace67e136b29cded3a4a7ec5fc952b0ddfa2632))
- Update architecture documentation with symbol aliases for reserved characters and modifier key support ([addc5d6](https://github.com/imbus/robotframework-PlatynUI/commit/addc5d6fa21a3e76af97d096a16a30cf39a7f55a))
- Enhance architecture documentation with linking macros and FFI details, update plan ([ebdf73f](https://github.com/imbus/robotframework-PlatynUI/commit/ebdf73f0bf4dedfc2bcbe7d17d87a3ce8bba136c))
- Enhance CONTRIBUTING.md ([d7304ed](https://github.com/imbus/robotframework-PlatynUI/commit/d7304ed9a481a2fe45bf02bed9368676d34800c8))
- Update README.md for clarity and structure, enhancing installation instructions and project description ([5a447e3](https://github.com/imbus/robotframework-PlatynUI/commit/5a447e39af2847ee6f3ba1551dc83d9ebd6f2cd5))
- Update AGENTS.md ([72eeae6](https://github.com/imbus/robotframework-PlatynUI/commit/72eeae6ebb88734808dcf595e79bcd270395418e))
- Refine plan ([650e34d](https://github.com/imbus/robotframework-PlatynUI/commit/650e34d9e936cb456eaad9b8e06d7853eb30fdb3))
- Enhance Windows cross-compilation instructions for GNU and MSVC toolchains ([99480cd](https://github.com/imbus/robotframework-PlatynUI/commit/99480cd162556961f34f2a1d402c822e6286b8a9))
- Capture platform init and pointer updates ([434ff27](https://github.com/imbus/robotframework-PlatynUI/commit/434ff27e2fde9e0d9a4ca42dffb1f8355c8f1b4c))
- Expand Windows platform section with detailed tasks for Pointer, Keyboard, Highlight, Screenshot, and UIAutomation integration ([17813ec](https://github.com/imbus/robotframework-PlatynUI/commit/17813eccaf92c6bfaa10a2fdb38e3b018dcd54ee))
- Update roadmap and implementation focus for cross-platform support ([0a6853f](https://github.com/imbus/robotframework-PlatynUI/commit/0a6853f625896c5692144f04c62cb61361c900d5))
- Update documentation for PlatynUI Runtime architecture and legacy analysis ([e0bbe70](https://github.com/imbus/robotframework-PlatynUI/commit/e0bbe708e78baf34851f0e1b5e1e3d83a8b78686))
- Implement comprehensive architecture and design documentation for PlatynUI Runtime ([6d6ea46](https://github.com/imbus/robotframework-PlatynUI/commit/6d6ea462c4b5c5adb2cca97b236e02684c54f998))
- Add error handling guidelines for Rust in AGENTS.md ([b591326](https://github.com/imbus/robotframework-PlatynUI/commit/b591326fc75d1cdd02ab593dfff9c2384f43ea88))
- Update comments in model.rs to use English and improve clarity in AGENTS.md ([e3f502a](https://github.com/imbus/robotframework-PlatynUI/commit/e3f502a38524900c4592a737094f0066b6d10283))
- Update xpath 2 plan ([1855790](https://github.com/imbus/robotframework-PlatynUI/commit/18557901b1fd4bc3359d48d96e1d89f1f01363c0))


### Features

- **BareMetal:** Add Wait Until Exists, Gone and Query keywords ([a3d5057](https://github.com/imbus/robotframework-PlatynUI/commit/a3d505779011e3e0b87be339c96048b94c154d89))

  Explicit synchronization keywords, the waiting counterparts to the
  non-waiting Query:

  - Wait Until Exists: wait for a selector to resolve to an element and
    return it (element-only).
  - Wait Until Gone: wait until a selector matches nothing, or a captured
    element becomes invalid; rejects value selectors.
  - Wait Until Query: wait until an XPath result satisfies an AssertionEngine
    assertion (same operators as Get Attribute); with no operator it waits
    for a truthy result. Rejects the transforming then operator; validate
    polls correctly.

  Per-call waiting is tuned via query_overrides only. Adds the dedicated
  ElementStillPresentError and a "Waiting explicitly" docs subsection.

  Covered by a mock RF suite and an egui acceptance suite verified on the
  compositor and X11 backends — including the captured-element "gone" path
  against the real AT-SPI provider, which the mock cannot exercise.

  Includes the OpenSpec capability spec (baremetal-waiting) and the archived
  change artifacts.

- **BareMetal:** Support configurable persistent input defaults in BareMetal ([f341e16](https://github.com/imbus/robotframework-PlatynUI/commit/f341e167d8e8102c2e679845aa1fd56dc6c2e4f6))

  - Allow configuring persistent keyboard, pointer settings, and pointer
    profile defaults when initializing the Robot BareMetal library.
  - Apply partial input default updates by merging with current runtime
    values so unspecified fields keep their existing behavior.
  - Keep API semantics clear by separating persistent settings from
    per-action overrides for keyboard input.
  - Align Python typing/stubs with runtime behavior, including pointer
    settings/profile aliases and pointer button typing for dict-like
    inputs.
  - Improve test usability for local Robot runs by wiring initialization
    defaults in the sample BareMetal suite setup.

- **BareMetal:** Enhance query handling with timeout and retry logic ([ee4a1d0](https://github.com/imbus/robotframework-PlatynUI/commit/ee4a1d011367208e1dd9dedf738fcac1713db540))
- **BareMetal:** Enhance UiNodeDescriptor with root management and add demo test case ([07b506a](https://github.com/imbus/robotframework-PlatynUI/commit/07b506aad7cdce0bb3a6a26ff672e24d02ab1bdf))
- **BareMetal:** Add auto_activate option and window activation control for pointer actions ([bb72d0d](https://github.com/imbus/robotframework-PlatynUI/commit/bb72d0d804d3f475c38e3d13ff4bb1641b5b76eb))
- **BareMetal:** Enhance highlight functionality to support multiple descriptors and optional rectangle input ([e83cf26](https://github.com/imbus/robotframework-PlatynUI/commit/e83cf266d4a21951a922e1965e9993c29ac22933))
- **adapter:** Wrap Element, ActivationTarget and Readable as attribute-only native patterns ([6357cf5](https://github.com/imbus/robotframework-PlatynUI/commit/6357cf5e0c74b0685863d670124f4748a816fd26))

  Move the three attribute-only patterns (Element, ActivationTarget,
  Readable) from ElementProxy mixins to UiNodeAdapter native wrappers,
  mirroring the WindowState approach. Generalises supports_pattern via a
  shared _ATTRIBUTE_ONLY_PATTERNS table keyed on a sentinel attribute.

  Extends the mock provider tree with IsReadOnly so Readable can be
  exercised end-to-end. Updates the design doc (Rev. 40) and migration
  status accordingly.

- **adapter:** Add WindowState pattern and native pattern wrappers ([07759fb](https://github.com/imbus/robotframework-PlatynUI/commit/07759fbea0988792d7bf7b9c228e44823c57e91d))

  Introduce WindowState pattern (is_active, is_topmost) to separate
  read-only window status bits from the Activatable action. Add nine
  native pattern wrappers in UiNodeAdapter (_NativeActivatable,
  _NativeWindowState, _NativeMinimizable, _NativeMaximizable,
  _NativeRestorable, _NativeCloseable, _NativeMovable, _NativeResizable,
  _NativeResponsive) so that window sub-patterns from the Rust split
  (Rev. 37) reach Python via the adapter without a WindowProxy.

  Window.is_active and Window.is_topmost now read through WindowState
  instead of relying on the Focusable.is_focused fallback. Test stubs
  get a WindowStateStub and 11 test files are migrated accordingly.

  Designdoc Rev. 38 documents the default-proxy hierarchy and the
  native-wrapper layer; the originally planned WindowProxy is
  dropped.

- **atspi:** Add EWMH helpers for X11 window management and integrate WindowSurface pattern ([c4903b0](https://github.com/imbus/robotframework-PlatynUI/commit/c4903b07a4c5ce8af20868a4fc8590b3313f4394))
- **atspi:** Add process ID resolution for application nodes ([babe870](https://github.com/imbus/robotframework-PlatynUI/commit/babe8704c84b8d93208593661b257bed286edcfe))
- **baremetal:** Let Wait Until Attribute Value poll like every wait ([a16a355](https://github.com/imbus/robotframework-PlatynUI/commit/a16a35581dc2bb50dfef24d012b6964367572aeb))

  - Under ignore_exceptions, the failure describes the last attempt.
    When it raised, the failure says how far it got (no element, the
    attribute not read, the value not checked) and quotes the error,
    instead of an older attempt's mismatch. A check that raises no
    longer reports "No element matched".
  - Under ignore_exceptions, a Set Root root that cannot be found fails
    the keyword with RootNotFoundError.

- **baremetal:** Let Query skip the root for absolute expressions ([39072b0](https://github.com/imbus/robotframework-PlatynUI/commit/39072b045b5c5e50b9185df26d4d7b8ac82c28a4))

  Query now looks the Set Root root up only for a relative expression,
  as every other keyword does. An absolute expression such as
  count(//Button) no longer waits for or fails on a root that cannot be
  found, and a missing root names the expression it kept from being
  evaluated.

- **baremetal:** Let Wait Until Query poll like every wait ([cb6fa8d](https://github.com/imbus/robotframework-PlatynUI/commit/cb6fa8da63cba1e4f93826f1e9e1ba5775cebf86))

  - An absolute expression is evaluated without the Set Root root.
  - An explicit root from another import is rejected before the first
    attempt, and one that is no longer valid ends the wait at once.
  - The truthy timeout names the last result ("was 0", "matched
    nothing").
  - Nothing is evaluated after the deadline. With an operator, the wait
    fails with AssertionError or TypeError that carries the timeout,
    instead of letting a raw error escape.
  - Under ignore_exceptions, the failure quotes the last error.

- **baremetal:** Let Wait Until Gone poll like every wait ([c806d39](https://github.com/imbus/robotframework-PlatynUI/commit/c806d390c2a54afc1e30fd90d2f0e00493326c1d))

  - Under ignore_exceptions, a Set Root root that cannot be found fails
    Wait Until Gone with RootNotFoundError after the root's own timeout,
    instead of ElementStillPresentError after the per-call timeout.
  - When its last attempt raised, it says that the element could not be
    confirmed gone and quotes the error, instead of "still present".
  - A captured element is named as Role "Name" #id.
  - A reference with neither a selector nor an element raises
    NoQueryError instead of counting as gone.

  The egui acceptance suite gains a test for a root of an ended
  application under ignore_exceptions.

- **baremetal:** Run the element lookup through one polling loop ([c21672f](https://github.com/imbus/robotframework-PlatynUI/commit/c21672fcd8bc736b7fa6b7b005a8054df9b22a92))

  Every keyword that takes an element, and the lookup of a Set Root
  root, now wait through one helper; the other waits follow in the next
  commits.

  - Every attempt looks the root up first, outside the errors that
    ignore_exceptions swallows. A root that cannot be found ends the wait
    with RootNotFoundError, and a root that is replaced during the wait
    is followed. The wait's clock starts once the root has been found.
  - Errors that waiting cannot fix end the wait at once, whatever
    ignore_exceptions says.
  - Under ignore_exceptions, a timeout ends its message with "The last
    error was: <Type>: <message>" and is chained to that error. This
    holds for the root's own lookup too.

- **baremetal:** Add Get Attribute Value and deprecate Get Attribute ([91a5a46](https://github.com/imbus/robotframework-PlatynUI/commit/91a5a460fef885f5cd0247c0e0d08d5540333c43))

  Get Attribute returns an attribute's value, not the attribute, so the
  keyword is now Get Attribute Value, which also pairs it with Wait
  Until Attribute Value. Get Attribute stays as an alias with the same
  behavior until it is removed before 1.0; Robot Framework's deprecation
  marker makes every use log a warning that names the replacement.

  The library documentation uses the new name throughout, and the
  roadmap records the removal of the alias.

- **baremetal:** Add Wait Until Attribute Value ([dda0b12](https://github.com/imbus/robotframework-PlatynUI/commit/dda0b1293d5e291a65848c21c906ccf9330c6046))

  Get Attribute reads and checks a value once, so suites awaited
  asynchronous state changes with Wait Until Query <selector>/@Attr.
  The new keyword waits until one attribute of one element satisfies an
  AssertionEngine condition, truthy without an operator, and returns the
  value the attribute had at that moment, also for matches.

  A selector is evaluated again on every attempt. A captured element is
  refreshed before each read and fails at once when it is gone. Usage
  errors fail at once, and the timeout error reports what the last
  attempt saw: no element, no such attribute, or the failed check.

- **baremetal:** Log keyword actions at DEBUG and accept Secret ([e0996fc](https://github.com/imbus/robotframework-PlatynUI/commit/e0996fc0f98fd7a4ec7b71008cb5b027d6cd73a1))

  - Every action keyword logs one DEBUG line through platynui.baremetal
    after it succeeds: the element as it was before the action, the point
    and where it came from, and the button, clicks or ticks. Keyboard
    keywords give only the length of the input.
  - The keyboard keywords accept Robot Framework's Secret, imported only
    where it exists; a Secret's error names only the position and the kind
    of failure. For a plain string, an error that hints at an escape adds
    its Robot Framework form.
  - A failed automatic activation is logged at DEBUG, naming the element
    and the error, and the action proceeds.
  - A root set by Set Root that is not found raises RootNotFoundError with
    its own text, and lookup errors no longer name internal types.
  - The library documentation covers the level names, the configuration
    warnings, Secret, the sequence syntax and how to report a problem.

  Behavior changes: the logger platynui.devices is now
  platynui.core.adapter_devices; a missing Set Root root fails with a new
  text; element lookup errors say element instead of UiNode.

- **baremetal:** Show native diagnostics in the Robot Framework log ([e7e3ab6](https://github.com/imbus/robotframework-PlatynUI/commit/e7e3ab670547064d753d578a1a566ee94f64a9f8))

  Robot Framework records library messages only from the thread that runs
  the keyword. OurDynamicCore.run_keyword therefore delivers queued native
  log records before a keyword runs (what native background threads logged
  since the last keyword) and after it returns (what the keyword itself
  caused). A native record then appears in the log of the keyword during
  which it was logged, or at the latest in the next PlatynUI keyword, at
  its own level: a native warning is an RF warning.

  PlatynUI.BareMetal gains the import argument native_log_level (error,
  warn, info, debug, trace). The native level is process-wide, so each
  library instance registers a request, the most verbose live request
  applies, and a library listener withdraws the request and delivers once
  more when the instance's suite ends. An invalid value fails the import
  naming the argument and the accepted values; an extension built before
  native logging fails it asking for a native rebuild.

  A pytest-driven Robot Framework run checks the records inside the right
  keywords at the right levels in output.xml. dev-docs/python-bindings.md
  explains where records go, when they are delivered and why, and which
  sources set the level. The tracing instructions name the extension as
  the third place allowed to install a subscriber and note that a native
  warn reaches the RF console. The change records the lane review and the
  pending Windows review.

- **baremetal:** Adopt RF's scope names and reuse scoped roots ([f5c3b54](https://github.com/imbus/robotframework-PlatynUI/commit/f5c3b545b307c454f4dab3b77d42e2fb13b8dbf9))

  Two changes to Set Root / Set Query Settings that belong together,
  because the second is only safe given the boundary the first draws.

  Behavior change: scope=SUITE no longer reaches child suites. It previously
  passed children=True unconditionally, which gave the keyword a scope
  Robot Framework has no equivalent for. Measured with RF 7: a variable
  set via Set Suite Variable in an __init__.robot reads <unset> in a child
  suite. The suite tree is now scope=SUITES, RF's own name for it
  (Var._get_scope maps SUITES to suite + children=True), so a directory
  fixture is still reachable — by asking for it.

  - scope accepts LOCAL, TEST, TASK, SUITE, SUITES and GLOBAL, routed
    through the matching VariableScopes setter. TASK and GLOBAL are new;
    RF's Literal conversion makes the names case-insensitive, as RF's own
    scope handling is.
  - A root that pins an element is validated once, on its whole chain, for
    every form the argument takes — fresh capture, restored root binding,
    or a selector root drilled into either. A foreign element raises
    ForeignNodeError at any scope (this closes the restore path, which
    bypassed the check and only failed on the next lookup); an own element
    is refused at SUITES/GLOBAL with UnsharableRootError, since a suite
    below builds its own runtime and could not find it again. Nothing is
    stored in either case. This replaces two read-time guards that were
    built and removed here, an import-args fingerprint and an owner check
    on read: with the write guarded, a reader across the boundary only
    ever sees selectors.
  - A scoped root now keeps the element it resolved to, while that element
    stays valid and belongs to the resolving instance. A root is looked up
    once per keyword on top of the keyword's own target, so it is
    repetition the suite never wrote, and unlike a target selector — whose
    re-evaluation is the observation a keyword makes — it names a
    container pinned on purpose. Target selectors stay uncached.
  - Consequently UiNode::is_valid is load-bearing, not a hint: its `true`
    default would leave such a root never resolved again, so the trait now
    says any provider handing out nodes with a real lifetime owns it.
  - A dead capture reports PinnedElementGoneError naming the element,
    instead of falling through to "has no query to resolve the node" —
    capturing is now the documented way to skip repeated lookups, so its
    failure has to be readable.

- **baremetal:** Add Pointer Scroll keyword ([f697b92](https://github.com/imbus/robotframework-PlatynUI/commit/f697b92b950b0e59eb4869fdcfd6789d30992ca8))

  Add a `Pointer Scroll` keyword to the BareMetal library: turns the mouse
  wheel by `ticks` notches in a `direction` (UP/DOWN/LEFT/RIGHT), over an
  element, at coordinates, or at the current pointer position. The keyword
  owns the sign/axis convention so callers never write signed deltas.

  Emitting real scroll for the first time exposed latent platform-layer
  bugs, fixed here so scroll works end-to-end on every backend:
  - runtime: chunk an axis whose `scroll_step` component is 0 by one notch
    (120) per step, not 1-unit micro-steps — that broke horizontal scroll
    everywhere (negligible on Wayland, rounded to zero wheel clicks on X11).
  - wayland compositor: inject `AxisSource::Wheel` + discrete `v120` (was
    `Finger` + continuous value) and translate the sign to Wayland's axis
    convention.
  - windows: negate the horizontal delta for `MOUSEEVENTF_HWHEEL` (Win32's
    horizontal wheel is right-positive, opposite to PlatynUI/X11).

- **baremetal:** Honor auto_activate in keyboard/focus/screenshot/highlight ([078dabc](https://github.com/imbus/robotframework-PlatynUI/commit/078dabc1121ba4b117ea75cf67b95e9c17e392b8))

  Until now only the pointer keywords raised the target's top-level window
  before acting (gated by auto_activate / a per-call activate). Extend the
  same gating to the keyword family that also depends on the right window
  being in the foreground:

  - focus, keyboard_type/press/release: bring the element's top-level window
    to the front before focusing/typing. Setting focus alone is app-local
    and does not guarantee the window is the desktop-active one, so input
    could otherwise land in the wrong window.
  - take_screenshot (element): raise first so an occluding window does not
    corrupt the captured pixels.
  - highlight: raise each element's window before outlining it; the call
    stays inside the per-descriptor try/except so a missing node is skipped,
    not raised.

  Each keyword gains a per-call activate override (bool | None), mirroring
  the pointer keywords and defaulting to the library's auto_activate; reuses
  the existing _maybe_bring_to_front helper.

- **baremetal:** Configurable query settings (init/keyword/scope) ([80d1b7f](https://github.com/imbus/robotframework-PlatynUI/commit/80d1b7f43f6fcdb42b139ad0cc2dfdcbd9e07569))

  The query wait/retry settings (timeout, retry_interval,
  ignore_exceptions) were a single hardcoded instance. Make them
  configurable on three layers, each mirroring an existing convention:

  - at import:   query_settings={...}                       (like pointer_settings)
  - per keyword: query_overrides={...}                      (like overrides)
  - per scope:   Set Query Settings ... scope=LOCAL/TEST/SUITE (like Set Root)

  Precedence, highest first: per-call > scope > import default > builtin.
  Each scope keeps its own QuerySettings instance in the Robot Framework
  variable ${PLATYNUI_QUERY_SETTINGS}, so RF's nearest-scope-wins
  resolution provides precedence and lifetime, and Set Query Settings
  returns the previous value for exact restore. A partial override merges
  onto the base visible from its target scope, never a narrower active one.

  Per-call overrides live on the (shared, cached) descriptor and are set
  unconditionally so they never leak between calls; they tune only the
  keyword's own target, while import and scope settings also cover the
  re-resolution of a Set Root root.

  QuerySettings becomes a frozen dataclass and QuerySettingsDict a partial
  TypedDict (like the native *OverridesDict types); merging uses
  dataclasses.replace.

  Also fixes a latent bug made reachable by exposing ignore_exceptions:
  when it swallowed a persistent evaluation error the wait loop skipped the
  timeout check and the sleep, spinning forever. The timeout is now
  honoured on every iteration, whether the attempt found nothing or raised.

- **baremetal:** Scope-aware Set Root via engine-level XPath context classification ([3b4b5de](https://github.com/imbus/robotframework-PlatynUI/commit/3b4b5defbd7d513a360af324de31f2dcf5c70994))

  Replace the incomplete leading-slash heuristic in Set Root with a real
  context-dependence classifier in the XPath engine, and overhaul the library
  documentation.

  Engine + bindings:
  - xpath: add Expr::is_context_dependent(), true when an expression reads the
    context node at the top level (relative path, ., 0-arity context function),
    recursing through if/for/let/sequence/filter/union; predicates and inner
    scopes don't count, so (//X)[1] and //X[.='y'] stay context-independent.
  - runtime + native: expose is_context_dependent(xpath) on the runtime crate
    and the PyO3 Runtime (+ .pyi stub).

  Set Root:
  - decide relative drilling vs absolute root via the engine classifier;
  - read/write the root in the requested scope directly instead of through
    BuiltIn (no stray logging, no RF variable-syntax resolution of selectors);
  - raise InvalidSelectorError on a malformed selector.

- **cli:** Use the shared log filter and warn when watch cannot see ([8448841](https://github.com/imbus/robotframework-PlatynUI/commit/8448841a369983988c4581e82c26a33f78cefbeb))

  --log-level takes the shared level names, case-insensitive, and its help
  no longer names the nonexistent PLATYNUI_LOG. watch warns when no active
  provider emits UI events, because it would wait without output.

  Behavior change: a single level no longer lowers third-party crates such
  as zbus, RUST_LOG does; error and off apply to every module;
  PLATYNUI_LOG_LEVEL takes a single level only.

- **cli:** Add global no-activate option to prevent window activation before actions ([ac00509](https://github.com/imbus/robotframework-PlatynUI/commit/ac0050969fadabf47c5a979885e420e2d7a65a9a))
- **cli:** Add snapshot command for exporting UI subtrees as XML ([60bf6b0](https://github.com/imbus/robotframework-PlatynUI/commit/60bf6b06dd2e0a7f6822fa803d452dc4aedb2051))
- **cli:** Enhance pointer commands to return element information on actions ([fde9ed3](https://github.com/imbus/robotframework-PlatynUI/commit/fde9ed394d2beef4d28d4768689dee8a54fafe2b))
- **cli:** Enhance pointer commands to support XPath expressions and optional point arguments ([2b8a8ae](https://github.com/imbus/robotframework-PlatynUI/commit/2b8a8aedf347bd1e5a0df2464e9ba4219efefc52))
- **cli:** Add focus command leveraging runtime patterns ([504a529](https://github.com/imbus/robotframework-PlatynUI/commit/504a529baf59f475c68bc8e91a07dba044599eb4))
- **cli:** Colorize query output ([704d3c4](https://github.com/imbus/robotframework-PlatynUI/commit/704d3c433e0a5ed865a18187f0e4afb86bb4c9c4))
- **cli:** Add watch command for provider events ([273b42d](https://github.com/imbus/robotframework-PlatynUI/commit/273b42df4708f72ea721b3cae44bcd55b7931084))
- **cli:** Add mock-backed provider listing ([bbaef5f](https://github.com/imbus/robotframework-PlatynUI/commit/bbaef5f1093588ba77fdcb7731622033d5c46ebc))
- **cli,xpath:** Readable XPath syntax errors with source-line caret ([bbfdf77](https://github.com/imbus/robotframework-PlatynUI/commit/bbfdf77e44c26201a323f93971cb4173fceb7a88))

  * cli: print top-level errors directly to stderr ("Error:" bold-red
    via owo-colors) instead of routing them through tracing.
  * xpath::Error Display: emit "(CODE): {message}" with bare local
    names (drop the err: namespace prefix) and adjust the runtime
    wrapper accordingly.
  * xpath::parser: replace pest's default Display dump with a
    compiler-style block (Syntax Error at line/column, source line,
    caret, inline detail). Map raw grammar rules to friendly labels
    and group operators/comparisons/type-tests so 20+ alternatives
    collapse to a handful.
  * deps: bump lru 0.17 → 0.18 (xpath) and winreg 0.52 → 0.56
    (inspector); refresh Cargo.lock.

- **compositor:** Exact popup interaction and picker under Wayland ([5b08d3e](https://github.com/imbus/robotframework-PlatynUI/commit/5b08d3e6bbb29cc85f685cc413c01c8af747f3cc))

  Two compositor-session gaps left by atspi-event-driven-tree and
  inspector-live-mouse-picker: AT-SPI popup extents are client-local on
  Wayland (clicks into open menus land offset), and the Inspector picker
  had no modifier source under the compositor — a leaked host DISPLAY
  even let the X11 reader bind the wrong seat.

  - control socket: new list_popups (global popup rects incl. cascade
    levels, parent window id/pid) and get_modifiers (seat ctrl/alt/shift/
    logo) commands
  - WindowManager gains a defaulted popups(pid) query; implemented by the
    PlatynUI IPC backend; the AT-SPI provider uses it for grafted
    popup-class nodes, matched by pid + size (most recent on ties), and
    falls back to toolkit extents elsewhere — X11/Windows/mock unchanged
  - Inspector modifier reader selection is explicit and compositor-first
    (control socket answering ping, then X11); the compositor now strips
    a leaked host DISPLAY from the session child env unless XWayland
    provides one
  - control socket robustness: process lines arriving together with EOF
    (write-then-close clients) and retry large responses on WouldBlock
    instead of silently truncating (screenshots of rich scenes)
  - the three Wayland-skipped submenu acceptance tests run unskipped; new
    ipc_tests cover both commands via a raw wayland-client popup fixture

- **compositor:** Handle GPU hotplug events ([af5a112](https://github.com/imbus/robotframework-PlatynUI/commit/af5a1120ee882da64fa16e176694c82ffca98be7))

  Track the udev device ID per GPU to correlate add/remove events.
  Implement full teardown on GPU removal: unmap all outputs, drop
  DRM/GBM resources, relocate windows, and notify output-management
  clients.  Log when additional GPUs are detected (multi-GPU not yet
  supported).

- **compositor:** Add connector hotplug detection and handling ([230cbd0](https://github.com/imbus/robotframework-PlatynUI/commit/230cbd0c0f396b8a27d157213e4b88f18268c986))

  Detect monitor connect/disconnect events at runtime and dynamically
  update the output layout.

  - Newly connected monitors are mapped to the right of existing outputs
  - Disconnected monitors are removed and their windows relocated
  - Output-management clients (wlr-randr, kanshi) are notified of changes
  - GPU hotplug (add/remove entire GPU) logged but not yet handled

- **compositor:** Improve DRM session handling and frame timing ([776a1c6](https://github.com/imbus/robotframework-PlatynUI/commit/776a1c60603e141274a516bced910926cd19a204))

  Properly suspend libinput and release DRM master on VT switch,
  reactivate both on switch back so the display resumes correctly.

  Send frame callbacks after VBlank instead of at render time, giving
  clients accurate presentation timing. Deliver wp_presentation
  feedback with real hardware timestamps from the DRM driver.

  Track per-surface scanout output and DMA-BUF feedback so clients
  can optimize buffer allocation for the active display.

- **compositor:** Clamp floating windows into visible area on output resize ([8a4f0fc](https://github.com/imbus/robotframework-PlatynUI/commit/8a4f0fc580f7737f74df3b3ba688316b79742b3b))

  When outputs shrink (e.g. winit window resize, wlr-randr mode change),
  normal floating windows could end up entirely outside the visible area
  with no way for the user to reach them.

  Add clamp_floating_windows_to_outputs() which runs at the end of
  reconfigure_windows_for_outputs() and repositions any floating window
  whose titlebar is no longer reachable. At least TITLEBAR_HEIGHT pixels
  must remain visible on each axis so the user can always grab and drag
  the window back. Maximized and fullscreen windows are skipped (already
  resized to fill the output). X11 surfaces are notified via
  x11.configure(). Windows are never resized, only repositioned.

  This matches the behaviour of GNOME/Mutter and KDE/KWin which ensure
  the titlebar stays accessible after monitor configuration changes.

- **compositor:** Overhaul DRM backend for full multi-monitor support ([c23fd92](https://github.com/imbus/robotframework-PlatynUI/commit/c23fd92f777da84d9483c55085f7e519b3a6af3b))

  Restructure the DRM backend to detect and expose all connected monitors,
  even when the GPU has fewer CRTCs than connectors. Outputs are now keyed
  by connector handle instead of CRTC handle, and each output holds an
  optional ActiveDrmCompositor that is present only when a CRTC is assigned.

- **core:** Add a transition latch, element descriptions, config checks ([8e4a631](https://github.com/imbus/robotframework-PlatynUI/commit/8e4a631890186f5420d683773fd3d411f004439e))

  The building blocks of the logging concept, all without a tracing
  dependency: core returns facts and the call site logs.

  - diagnostics::Transitions records which subjects are failing. failed()
    tells whether a failure starts or continues an episode; recovered()
    and retain() end it, and retain() returns the subjects it forgot. A
    condition that recurs on every call is thereby warned once per
    episode.
  - ui::describe gives an element's one-line description, Role "Name" #Id,
    with the name cut to 60 characters and then escaped.
  - ConfigMap::unknown_keys and try_str, try_bool, try_i64 and try_map
    tell a mistyped setting (ConfigTypeMismatch) apart from an absent one,
    so that each component can report its own section.
    RuntimeConfig::try_platform_backend does the same for
    platform.backend.

- **core:** Add common control:Description attribute ([6640351](https://github.com/imbus/robotframework-PlatynUI/commit/6640351ac6bbb532680dba83bb41dc87ef222a79))

  Surface the accessible description as a provider-independent
  control:Description attribute alongside Role/Name/Id. Sourced strictly
  from the platform accessible-description property (UIA FullDescription,
  AT-SPI Accessible.Description) with no HelpText fallback, and emitted
  only when non-empty (the control:Id precedent).

  - core: common::DESCRIPTION constant + UiNode::description() default None
  - provider-mock: spec-driven descriptions (NodeSpec/XML), gated emission
  - provider-atspi: StdAttrKind::Description slot + resolve_description()
  - provider-windows-uia: map::get_description (UIA_FullDescriptionPropertyId),
    DescriptionAttr, gated iterator slot + attribute() fast-path arm
  - native/python: description getter, Adapter/Context read-only property
  - test-app-egui: set AccessKit description on the Click Me button
  - acceptance: egui Query suite verifies control:Description round-trip
  - docs: architecture.md common-attribute table, mapping, checklists

- **core:** Add provider-independent Java app classifier ([7949203](https://github.com/imbus/robotframework-PlatynUI/commit/7949203d88c791f61dc280e3e741abe79279ed02))

  Introduce platynui_core::platform::java: a JavaClassifier platform-bundle
  capability (is-JVM / toolkit / native-a11y reachability per top-level
  window), pure signal->classification logic, and a shared warn-once
  "JVM window absent from native accessibility" diagnostic.

  - platform-windows backend: GetClassNameW toolkit prefixes + Toolhelp
    jvm.dll scan (TTL-cached per PID); reachability from window claims
  - UIA and JAB window nodes surface native:IsJvm, native:JvmToolkit,
    native:JvmAccessibilityReachable; runtime injects the classifier
  - provider-jab delegates its SunAwtSuspect message to the shared
    diagnostic (enumeration untouched)
  - live scenario: bridge-on Swing classifies reachable, bridge-off fires
    the diagnostic once and shows not-reachable facts on the UIA shell

- **core:** Add window-claims registry and TextEditable pattern ([509f2a0](https://github.com/imbus/robotframework-PlatynUI/commit/509f2a045115f314d0698eccde7e4f1547d25f40))

  Add a process-wide window-claims registry (platform::window_claims) so
  one provider can mark a native top-level window as its own and other
  providers skip it, keeping each window single in the merged desktop
  tree. Add the TextEditable runtime pattern (TextEditableAction) for
  programmatic text replacement, plus the client-pattern attribute-name
  constants (TextEditable/Toggleable/StatefulValue/Selectable/
  SelectionProvider/Expandable) the JAB provider emits.

- **core:** Add RuntimeConfig and PlatformFactory foundation ([2a3f1d3](https://github.com/imbus/robotframework-PlatynUI/commit/2a3f1d37e2d6faab42468b9746934714b1e0b742))

  Additive foundation for per-runtime platform sessions (change
  per-runtime-platform-sessions, tasks 2.1-2.4):

  - config: RuntimeConfig + ConfigValue/ConfigMap with typed accessors,
    id-keyed platform/providers sections, tolerant type-mismatch -> None.
  - platform: PlatformFactory trait + PlatformBundle (Arc<dyn> devices)
    + inventory registration + register_platform_factory!.

  Nothing consumes these yet, so the workspace stays green. The Arc<dyn>
  bundle fields (vs the Box originally sketched) avoid a self-referential
  borrow once the runtime's PointerEngine holds the device; design D1/D2
  updated. cargo test -p platynui-core green; clippy/fmt clean.

- **core:** Add Selectable, Expandable, HasEditor and ItemContainer patterns ([22e3ec9](https://github.com/imbus/robotframework-PlatynUI/commit/22e3ec94d9e2cc7524318f64ee2661236a19b818))

  Introduce four pattern ABCs required for item-based UI controls:
  Selectable (selection state), Expandable (expand/collapse),
  HasEditor (in-place edit lifecycle) and ItemContainer
  (item/row/column counts).

- **core:** Chain PatternProxyFactory in RuntimeAdapterFactory ([7b16b87](https://github.com/imbus/robotframework-PlatynUI/commit/7b16b87697485ca7cf23c59f7c6f0fb68b4d728a))

  Promote AdapterProxy to an Adapter subclass so the resolution
  pipeline can return Adapter | None even when a registered proxy
  wraps the result. RuntimeAdapterFactory._wrap now calls
  PatternProxyFactory.find_proxy_for after constructing the
  UiNodeAdapter, activating Designdoc \xa74.4 step 3 that was a
  Phase-2 leftover. The redundant AdapterFacade alias is removed
  in the same go: with the subclass relationship in place the union
  collapses to plain Adapter.

- **core:** Auto-register context classes and warn on duplicate criteria ([f295dd2](https://github.com/imbus/robotframework-PlatynUI/commit/f295dd2bcf78f398ae1b45d5e85403e144971f56))

  Context and pattern-proxy registration now happen automatically via
  `__init_subclass__` (with `register=False` opt-out and abstract-class
  auto-skip), removing the need for explicit `@context` decorators on
  concrete page-objects. Equal-criteria registrations from different
  classes emit a new `DuplicateRegistrationWarning` so accidental shadow
  registrations surface early; same-class re-registration stays silent.

  The criteria-equality helper in `core/_criteria.py` normalises
  `re.Pattern` and nested dicts so the comparison is robust across
  re-imports.

- **core:** Add ElementDescriptor with swappable root-element storage ([d1979d7](https://github.com/imbus/robotframework-PlatynUI/commit/d1979d7c8745dbf3d9dd4fc1da29ae8663739028))

  ElementDescriptor[PatternT] is the lazy element reference used as
  Robot keyword argument type. It wraps either a Locator (resolution
  deferred until call) or an already-resolved ContextBase. With
  full_context=True the cached context is upgraded to the best-matching
  ContextBase subclass via ContextFactory.

  PatternT is a phantom marker (bound=PatternBase, default=PatternBase)
  that lets ElementDescriptor[patterns.Activatable] be registered as a
  distinct Robot converter; the actual pattern check is the keyword's
  responsibility. RootElementDescriptor is a subclass whose convert
  ignores the ambient root.

  The Robot variable ${PLATYNUI_ROOT_ELEMENT} backs the root-element
  storage in the Robot library, but core/ stays Robot-free: a swappable
  storage hook (set_root_element_storage / reset_root_element_storage)
  defaults to a single in-process slot. This is the foundation for
  sharing the root variable between PlatynUI and BareMetal once
  BareMetal moves to a variant of this descriptor (see design §13.2).

  17 tests cover construction, full/bare resolution, registry pick,
  UnknownContext fallback, explicit context_type override, parent
  chains, convert with/without ambient root, RootElementDescriptor,
  the storage-hook override+reset path, and _GenericAlias forwarding.

- **core:** Add ContextBase, ContextFactory and @context decorator ([8259a83](https://github.com/imbus/robotframework-PlatynUI/commit/8259a832ebc27fd9d94fe60ff7fc108aa903cb44))

  ContextBase is the page-object base: it holds a Locator, a parent
  context, and the cached Adapter resolved through adapter_factory.
  ContextFactory keeps the registry of page-object classes and picks
  the best-matching subclass for a freshly resolved adapter via
  WeightCalculator. UnknownContext is the fallback when no entry scores.

  Two equivalent registration routes share _register_context_class:
  the @context(role=..., framework_id=..., ...) decorator and the
  class-keyword form 'class Button(ContextBase, role=...): ...' via
  __init_subclass__. Abstract intermediates without kwargs are not
  auto-registered.

  ContextBase pass-through reads delegate to the adapter; element
  search delegates to adapter_factory.current.find_one/find_all. The
  parent_is_root_like flag is hard-coded to False until Phase 4
  (Desktop) reactivates it.

  38 tests cover registration paths, weighting, locator inheritance,
  adapter caching, parent/children traversal, and search delegation.

- **core:** Add AdapterFactory singleton with RuntimeAdapterFactory default ([badba55](https://github.com/imbus/robotframework-PlatynUI/commit/badba55dbb558d781df16c32b1439c5d272cbf67))

  Introduce AdapterFactory ABC, RuntimeAdapterFactory backed by the
  native runtime, and the process-wide adapter_factory accessor
  (lazy build + sealing + scope-bound override mirroring runtime).
  UiNodeAdapter exposes the wrapped native handle via a public
  native_node property; non-node XPath results raise the new
  InvalidResultTypeError(TypeError).

- **core:** Add element-relative mouse and keyboard proxies ([76c9b96](https://github.com/imbus/robotframework-PlatynUI/commit/76c9b962f21ae8acbe6d19e50ed512e107ab385a))

  Add MouseProxy/KeyboardProxy together with adapter-bound concrete
  implementations and an ActivationTarget pattern so callers can drive
  input against a UI element without dealing with absolute coordinates.

  Coordinate resolution accepts a Point offset, a VirtualPoint, or one of
  nine predefined Anchor positions plus optional x/y nudges. AdapterMouseProxy
  chooses the default click position from the adapter's patterns
  (ActivationTarget area centre -> activation point -> Element default)
  and logs the activation hint on DEBUG when present.

  Pre/post hooks (before_action/after_action) provide the extension
  point for the upcoming element layer to add focus and verification
  checks.

- **core:** Add process-wide Runtime singleton (Rev. 20) ([635258e](https://github.com/imbus/robotframework-PlatynUI/commit/635258eacc66eccc11c9885bdcdfcd53f6a1596b))

  Introduces a singleton accessor 'runtime' (class Runtime) in
  PlatynUI.core.runtime that wraps a single platynui_native.Runtime per
  process. Adapters, device proxies, future Robot keywords, the BareMetal
  helper and the inspector all share the same provider cache, pointer/
  keyboard profiles and desktop view through it.

- **core:** Add adapter facade for Python library (Phase 2) ([2b1fb3a](https://github.com/imbus/robotframework-PlatynUI/commit/2b1fb3a4963633e5409cd9781691a04670a288cc))

  Introduces the adapter abstraction layer that decouples Python patterns
  from any specific UI tree backend. Enables both native Rust-backed
  adapters and test-time stubs/spies via a uniform composition API.

  - Adapter ABC with template-method _resolve_pattern hook
  - AdapterProxy for composition-based pattern overlays (test seams)
  - UiNodeAdapter wrapping platynui_native.UiNode (the only production
    technology); UiNodeTechnology singleton
  - PatternBase.pattern_name now ClassVar for static identification

- **core:** Port Python core foundation (Phase 1) and align Rust attributes ([f64b441](https://github.com/imbus/robotframework-PlatynUI/commit/f64b441a6f4d489dd9a1f4d19843bcfcf4d14744))

  Migrate the Python-side foundation of the legacy PlatynUI project into
  this Rust-based workspace and align the Rust attribute layer with the
  new Python pattern hierarchy.

  Python (src/PlatynUI/core/, tests/PlatynUI/):

  - Add core foundation modules: ensure, exceptions, locator, predicate,
    settings, technology, types, wait, weight_calculator (Phase 1 of the
    migration plan, §10 of docs/python-library-design.md).
  - Add consolidated pattern ABCs in core/patterns/ (Element, TextContent,
    TextEditable, Clearable, Toggleable, Activatable, Focusable) mirroring
    the Rust capability groups in crates/core/src/ui/attributes.rs.
  - Locator supports three input channels (typed snake_case fields,
    free-form attributes dict, free-form kwargs) with explicit conflict
    detection; default attribute namespace is `control`.
  - @locator is a real decorator function (not a Locator alias):
    class-decorator form attaches __locator__; method/property form is a
    LocatorMethodDescriptor stub that raises NotImplementedError until
    ContextBase lands in Phase 3.
  - 128 pytest passing; ruff, mypy, pyright clean.

  Rust (Wire-breaking, project unreleased):

  - Rename attribute IsOffscreen -> IsInView and invert the semantics
    ("not visible in viewport" -> "visible in viewport"). Default fallback
    flips from false to true. Touches core attributes/identifiers/testkit,
    runtime desktop root, AT-SPI provider, and Windows UIA provider.
  - Extend pattern_ids with TEXT_EDITABLE, CLEARABLE, TOGGLEABLE.
  - 1980 nextest passing; clippy clean.

- **core:** Introduce is_valid method to check node availability ([6a82573](https://github.com/imbus/robotframework-PlatynUI/commit/6a82573dacd55a8b701ed05c0f824f4ea6cca3b1))
- **core:** Streamline keyboard device api ([931197b](https://github.com/imbus/robotframework-PlatynUI/commit/931197b8638aee9bc71e6b1885a4586146331c67))

  Complete keyboard device contract cleanup (Plan step 15): provider now exposes key_to_code + send_key_event only; documentation and checklists updated.

- **core:** Standardize runtime id scheme ([572df01](https://github.com/imbus/robotframework-PlatynUI/commit/572df018dc818a2c13942c72786daea6a57d86ed))
- **core:** Add lazy pattern resolution ([0ed0f8b](https://github.com/imbus/robotframework-PlatynUI/commit/0ed0f8bdf8ccdbba37d6a1811aa29312ae070f22))
- **core:** Add runtime pattern helpers ([4d36c43](https://github.com/imbus/robotframework-PlatynUI/commit/4d36c43dbb9375ce3347e451737c03fbd016c2bd))
- **core:** Support structured ui values and flatten geometry ([d22c8aa](https://github.com/imbus/robotframework-PlatynUI/commit/d22c8aac4ca06c32f16a12464ca43208a1239c67))
- **docs:** Update provider checklist and implementation plan for UiNode and UiAttribute traits ([f0c4189](https://github.com/imbus/robotframework-PlatynUI/commit/f0c4189ca32ff49200efe6cc66442cef998e6f49))
- **docs:** Enhance architecture and implementation details for UiNode and XPath integration ([13dcc62](https://github.com/imbus/robotframework-PlatynUI/commit/13dcc627bc3dbc5f8d0c47d5d364ed04fa41f1f9))
- **eis:** Propagate compositor's active XKB keymap to EIS clients ([6e8f45a](https://github.com/imbus/robotframework-PlatynUI/commit/6e8f45a85e1b5e1dc12eb6d6e5a9c309ddc9aaec))
- **eis-test-client:** Add standalone EIS test client ([bfec60f](https://github.com/imbus/robotframework-PlatynUI/commit/bfec60f7cc762fa4f675b56100f3c20fdb0cb8ca))

  Standalone binary for validating the EI protocol against compositors
  (GNOME/Mutter, KDE/KWin, and our own Wayland compositor).

- **evaluator:** Expose new streaming evaluation function in public API and add convenience function for streaming evaluation ([376e6b3](https://github.com/imbus/robotframework-PlatynUI/commit/376e6b398afda4973f8c99acf6a5d6c98313982b))
- **inspector:** Use the shared log filter and forward log records ([e99fccf](https://github.com/imbus/robotframework-PlatynUI/commit/e99fccf8cde02f3baa256bc2892b029deab40e78))

  --log-level takes the shared level names, case-insensitive, and the log
  records of the GUI stack pass through the same filter as PlatynUI's own.

  Behavior change: a single level no longer lowers third-party crates such
  as eframe and wgpu, RUST_LOG does; error and off apply to every module;
  PLATYNUI_LOG_LEVEL takes a single level only.

- **inspector:** Add Wayland headerbar window controls ([9fa8539](https://github.com/imbus/robotframework-PlatynUI/commit/9fa8539f2a6093880f98478ac73a0fed10f3f874))
- **inspector:** Follow the system light/dark theme ([2aea262](https://github.com/imbus/robotframework-PlatynUI/commit/2aea26273940463087dd271c2214a4bf4daf7e79))

  Implements the inspector-theme OpenSpec change (specs synced; the
  acceptance-lane task is still being verified):

  - Persisted theme preference (System default / Light / Dark) in
    inspector.ron, editable as a radio row in the Settings dialog,
    mapping verbatim to egui's ThemePreference.
  - Linux system-theme detection via the XDG Desktop Portal
    (org.freedesktop.appearance color-scheme) with live updates through
    the SettingChanged signal: a small zbus blocking watcher feeds the
    value into egui's fallback_theme, which is only consulted where the
    windowing system reports no theme — exactly Linux; Windows/macOS
    keep following winit's report.
  - "No preference" (0) counts as light per freedesktop/GTK convention
    (GNOME and DankMaterialShell report light mode as 'default'/0 —
    verified live under niri/DMS); only a truly absent or unreadable
    signal falls back to dark via a NO_SIGNAL sentinel, keeping the
    portal-less test lanes deterministic.
  - Removes the forced Visuals::dark() left over from the Slint→egui
    migration; fallback_theme = Dark is set explicitly instead.
  - Ephemeral override for test runs: --theme system|light|dark CLI flag
    plus PLATYNUI_INSPECTOR_THEME env var (CLI > env > persisted),
    never written back to the settings file; the Settings dialog notes
    when a run is pinned.
  - Unit tests for override precedence, portal-value mapping, and the
    settings round-trip incl. old-file compatibility.

- **inspector:** Redesign header as themed toolbar, segment status bar ([b676dbe](https://github.com/imbus/robotframework-PlatynUI/commit/b676dbe73a4f0c78169d9a6dcfcac7030199e2b1))

  Implements the inspector-toolbar-redesign OpenSpec change (all 20 tasks;
  delta specs synced to openspec/specs/):

  - Toolbar as a real egui top panel below the menu bar: Pick Element
    toggle, Refresh Node, Refresh Subtree, Highlight Node, right-aligned
    Always-on-Top pin — replaces the unthemed picker strip and the
    overloaded search row (320 px budget gone, field takes the row).
  - Icons: white-normalized Phosphor SVGs (MIT) under assets/icons/,
    rendered via egui_extras' svg feature and tinted with the text color;
    toggles switch to the fill weight while active.
  - Every toolbar control carries a stable AccessKit author_id (common
    @Id: picker-toggle, refresh-node, refresh-subtree, highlight-node,
    always-on-top), a mode-independent accessible name, and an
    "Action — Shortcut" tooltip.
  - New toolbar_style setting (IconsOnly default, IconsAndText) persisted
    in inspector.ron and editable in the Settings dialog; identical
    toolbar height in both modes.
  - Status bar split into a transient left segment (activity indicator,
    search/result statuses, new "Picked: <name>" pick announcement) and a
    persistent right segment showing the picker state; inline picker
    hint labels removed.
  - Acceptance: new inspector_toolbar.robot suite (stable IDs, slimmed
    search row, status-bar segments); inspector_picker.robot locates the
    toggle by @Id instead of @Name. Unit tests for the settings
    round-trip and the picker-state text.

- **inspector:** Configurable picker combination with persisted settings ([866aa92](https://github.com/imbus/robotframework-PlatynUI/commit/866aa92934f88d63262df426704030b76451c5b1))

  Add a Settings dialog (File -> Settings...) to configure the live
  picker's activation modifier combination (default Ctrl+Alt+Shift). The
  pure picker state rejects an empty set and stops picking on
  reconfiguration; the combination persists via eframe storage in the
  platform config dir ($XDG_CONFIG_HOME/platynui/inspector.ron), storing
  only explicit settings (no egui memory or window geometry).

  PLATYNUI_INSPECTOR_SETTINGS_PATH overrides the settings file path; the
  inspector-picker acceptance suite sets it to stay hermetic against the
  user's real configuration (a reconfigured combination on the host would
  otherwise break the suite's default-combination pick).

- **inspector:** Live mouse picker (element-at-point hit-test) ([4ffdfcc](https://github.com/imbus/robotframework-PlatynUI/commit/4ffdfcc9c0f2f66cf98f9b4cc2bb949b46a59f64))

  Add a reverse hit-test capability (which element is at a screen point) across
  the stack, and a live mouse picker in the Inspector: while armed, holding the
  configured modifiers (default Ctrl+Alt+Shift) continuously resolves, selects and
  highlights the element under the cursor, Accessibility-Insights style.

- **inspector:** Add configurable search result limit ([dd616d8](https://github.com/imbus/robotframework-PlatynUI/commit/dd616d83ffce96a313f893a0d20ab3b9bda8617e))

  Stop Inspector XPath searches when the configured UI result limit is reached to protect interactive sessions from overly broad queries.

  Add CLI and environment overrides, surface a dedicated limited-search status, and update the Inspector documentation and async hardening roadmap.

- **inspector:** Add renderer selection options ([fbda1c6](https://github.com/imbus/robotframework-PlatynUI/commit/fbda1c68d07597e9ddb3c3fc6f445db88b2bda9a))

  Enable the glow renderer alongside wgpu and expose renderer selection through CLI and environment variables. Scope the hardware acceleration option to the glow renderer and document the relevant wgpu environment variables for VM and driver diagnostics.

- **inspector:** Migrate background tasks to egui-async ([45b6b01](https://github.com/imbus/robotframework-PlatynUI/commit/45b6b011192d0d25c3dd91a47695fca321682626))

  Replace the custom automation worker pool with egui-async bindings for
  initial load, search, reveal, selection, and highlighting. This keeps
  provider work off the UI thread while simplifying task state and
  cancellation handling.

  Move search and result status into the global status bar, make Enter
  restart the current XPath search, and keep result browsing separate from
  tree reveal.

  Add compact XPath error feedback in the search field with an inline
  warning icon, automatic preview, full error popup, copy support, and
  cleanup of stale error state.

  Update the Inspector documentation and add the improvement roadmap for
  the remaining UX and async hardening work.

- **inspector:** Use OS default font and honor Windows text scaling ([881777d](https://github.com/imbus/robotframework-PlatynUI/commit/881777dce22ef9ccc2196fa3ff3c1176180c4717))

  Switch inspector typography to use the OS-resolved UI font as the
  primary egui font instead of fallback-only behavior. Keep egui defaults
  as a safe fallback when system font detection is unavailable.

  On Windows, read Accessibility TextScaleFactor and apply it as zoom so
  the inspector respects user-configured text size (for example 150%).
  Re-apply font setup when DPI scaling changes at runtime.

- **inspector:** Use system font as fallback for UI rendering ([0b24531](https://github.com/imbus/robotframework-PlatynUI/commit/0b24531bec021acc51f333d2f62504926b708e21))

  Add `egui-system-fonts` as a dependency and configure it in the
  egui context setup. The built-in egui proportional font remains
  the primary face so the inspector is always readable. The detected
  system font (e.g. Segoe UI on Windows, San Francisco on macOS) is
  added as a fallback, covering glyphs not present in the embedded
  font such as CJK characters and locale-specific symbols.

- **inspector:** Group attributes view by namespace ([69ab8f0](https://github.com/imbus/robotframework-PlatynUI/commit/69ab8f087aea962a916fe6425410819a7c8636d3))

  Add a grouped view to the Inspector's Attributes pane, similar to a
  Windows Explorer Details list with collapsible category headers.

  * Add a Grouped/Ungrouped toggle to the Attributes toolbar.
  * In Grouped mode, attributes are partitioned by namespace and rendered
    under full-width, clickable header rows showing a chevron, the
    namespace name and the item count, separated by a faint background
    band. Headers can be collapsed; the collapsed state is tracked per
    group in `collapsed_attribute_groups`.
  * Pinned attributes are surfaced in their own "Pinned Attributes"
    section above the regular groups.
  * Keep everything in a single TableBuilder so header and data rows
    share the same column widths and the same scroll area. The body is
    driven by a `BodyEntry` enum (Section / Namespace / Attribute) and
    rendered via `body.heterogeneous_rows`, so per-row heights (multi-
    line values, header bands) can be computed up front.
  * Reuse the chevron painter from the tree view so the look matches the
    rest of the inspector.

- **inspector:** Add pinned attributes to attributes pane ([af7033a](https://github.com/imbus/robotframework-PlatynUI/commit/af7033a26dc1f229ed489dde18f9134a40379f32))

  Allow users to pin frequently used attributes from the context menu so
  they stay above the regular sorted rows.

  Keep pinning session-local and preserve existing filtering, sorting, and
  copy behavior in the attributes view.

- **inspector:** Add property filter to the inspector pane ([242900c](https://github.com/imbus/robotframework-PlatynUI/commit/242900ca0002fbabae37dd582996016b4b315e53))

  Add a text filter to the properties panel and match against
  namespace:name, value, and type while preserving the existing sort
  order.

  Show filtered result counts, add a clear action for the filter input,
  and update the Inspector documentation to describe the new properties
  workflow

- **inspector:** Improve results panel interaction flow ([f5a7653](https://github.com/imbus/robotframework-PlatynUI/commit/f5a765335425a9305d85c01b4e27f8ff94736fe6))

  Make result browsing focus-only by default and require explicit reveal
  via Enter or double-click. Add result context menu actions for reveal,
  highlight, and copying label, runtime id, attribute value, and full
  result text.

  Extend search result helpers and ViewModel support for result
  highlighting, prevent the global highlight shortcut from stealing focus
  from the results panel, and update the Inspector docs to describe the
  new workflow.

- **inspector:** Add inspector menus and text selection actions ([5ee7a74](https://github.com/imbus/robotframework-PlatynUI/commit/5ee7a740dc797b7fabf36ced6d6b04f22fe237b4))

  Add a command-driven action layer for the inspector and expose search,
  refresh, subtree refresh, highlight, expand, and collapse through the
  menu bar, toolbar, context menus, and keyboard shortcuts.

  Render menu shortcuts with egui's shortcut_text API, keep Escape from
  dropping search-field focus, and refactor highlight handling into
  reusable ViewModel helpers.

  Improve the properties pane context menu with copy/select-all actions
  for text cells and preserve text selection when opening the menu with
  the secondary mouse button.

  Restore scroll-to-focused behavior in the tree view when revealing a
  node from the results list.

- **inspector:** Add About dialog ([3bc8639](https://github.com/imbus/robotframework-PlatynUI/commit/3bc863905707e6dd7f72fd71c46f2f8e7475b04e))
- **inspector:** Add application icon ([23abda6](https://github.com/imbus/robotframework-PlatynUI/commit/23abda664370d5acb7048d4dcdbbe0e92d384778))

  Display the PlatynUI logo in the window title bar, taskbar, and
  Windows executable. A placeholder icon is included until a final
  design is ready.

- **inspector:** Add keyboard-navigable results panel with async tree sync ([6c43317](https://github.com/imbus/robotframework-PlatynUI/commit/6c433176e04d1d61e445c48ace0edf8cf7cad82c))

  Redesign the results panel as a dedicated module with full keyboard
  navigation and click-to-select. Selecting a result automatically
  reveals the corresponding node in the tree view without blocking
  the UI. Switching quickly between results cancels any pending sync.

- **inspector:** Enhance search bar with multiline support and dynamic height ([1513ea6](https://github.com/imbus/robotframework-PlatynUI/commit/1513ea6c62f0cd4caa5b84d7d842668f290e2199))
- **inspector:** Stream XPath search results without blocking the UI ([07c8ef4](https://github.com/imbus/robotframework-PlatynUI/commit/07c8ef4934d140584e74f4f1992d79859d685841))

  Add non-blocking, cancellable XPath evaluation to the inspector.
  Previously, `evaluate_xpath()` called `runtime.evaluate()` synchronously,
  which materialized all results and froze the GUI during long-running
  queries.

- **inspector:** Implement caching and refreshing of nodes ([035b6e2](https://github.com/imbus/robotframework-PlatynUI/commit/035b6e29f9d635b41691a387b421018028dc676d))
- **inspector:** Add some custom components for split layout and treeview ([5ecb57a](https://github.com/imbus/robotframework-PlatynUI/commit/5ecb57a7a0426ad78b8a3e0225d68258ecdc4f60))
- **java-agent:** Serve every toolkit world, not just the agent's own ([4ee9fc7](https://github.com/imbus/robotframework-PlatynUI/commit/4ee9fc796d30253239937f46fadbb608e40b34ba))

  AWT scopes `Window.getWindows()` and `EventQueue.invokeLater` to the
  calling thread's `AppContext`. Java Web Start and applet runtimes give each
  hosted application one of its own, so the agent saw an empty desktop in
  precisely the applications it exists for — measured against a real
  OpenWebStart 1.14 target, along with the sandbox that killed it before it
  could report anything.

  Completes java-agent-web-start; the design records what was measured and
  which two of its assumptions the measurements overturned.

- **java-agent:** Read the Swing/AWT tree from inside the JVM ([311ddda](https://github.com/imbus/robotframework-PlatynUI/commit/311ddda9742c05f7360b3b5e99559edd41e97a73))

  The agent could reach a JVM but not describe its UI. This adds the
  Swing/AWT adapter: the instance tree as the spine, the accessible view
  as enrichment, and virtual children grafted where components end.

  Reading from inside is what closes the Access Bridge's fidelity gap. The
  JDK aliases every JTable cell to one shared renderer, so through JAB a
  cell has a volatile name, no bounds and no stable identity. Here the
  cell's value comes from the model and its rectangle from getCellRect —
  measured against the fixture, every cell carries its own name, its own
  bounds and its own selection state.

  Virtual children get interned per (owner, position) rather than
  registering the JDK's wrapper, which is a fresh object on every lookup:
  registering that would hand the same cell a new id per read, the exact
  opposite of the identity-stable ids this exists to provide.

  Notable details:

  - `Component.getName()` travels only when `nameExplicitlySet` says the
    application set it. AWT *manufactures* one for every unnamed Frame
    ("frame0"), so reporting it blindly renames every Swing window.
  - Coordinates convert via GraphicsConfiguration to the physical-pixel
    wire contract. The naive per-monitor translate cancels; see
    SwingGeometry for why one multiplication is the whole conversion.
  - The native window handle comes from the AWT peer. On Java 9+ that
    needs java.awt *and* sun.awt.windows opened, and the command-line
    remedy is unavailable to a design built on attaching to a JVM somebody
    else launched — so the agent opens them itself via
    Instrumentation.redefineModule, which is what JEP 261 gave agents that
    power for. Measured: no --add-opens needed on any JDK.
  - No text write and no highlight. Text is typed (`text-input-policy`);
    drawing is the platform's job, and what JAB lacks for a cell is
    *bounds*, not a way to draw.

- **java-agent:** Inject and talk to an in-JVM Java agent ([09d97a6](https://github.com/imbus/robotframework-PlatynUI/commit/09d97a63444479ba4d038f3b7fee0e40df851a23))

  Native accessibility leaves hard gaps for Java UI toolkits: JAB aliases
  every JTable cell to one shared renderer, and JavaFX has no accessibility
  at all on Linux. An in-JVM agent reads the toolkit's own in-process model
  and bypasses both. This is the toolkit-neutral foundation — getting the
  agent into a JVM, talking to it, shipping it. It surfaces no UI node; the
  toolkit adapters follow.

  - java/agent: one JAR, both entry points (`premain` for `-javaagent`,
    `agentmain` for attach), Java 8 bytecode, and no dependencies at all —
    it runs inside somebody else's process, so every extra jar would be one
    the target application did not ask for. Loopback NDJSON-RPC (forced,
    not chosen: Java 8 has no Unix sockets and cannot serve a named pipe)
    behind an owner-only handshake file carrying port, token, toolkits and
    version. Weak-reference element registry with a cheap per-element
    liveness answer, so an adapter can report node validity honestly.
    Toolkit detection reads the loaded-class list rather than probing,
    because `Toolkit.getDefaultToolkit()` would initialise AWT in an
    application that never had it.
  - crates/java-agent: the JVM attach protocol spoken natively, so the test
    host needs no JDK and we ship no unsigned foreign binary doing
    OpenProcess/CreateRemoteThread. Attach is the primary path — Java
    applications are launched by scripts, installers or Web Start, and the
    Inspector's core use is looking into an application that is already
    running, where `-javaagent` is impossible by definition. Injection is a
    one-time control plane; all traffic goes over the agent's own
    connection, with a deadline on every read.
  - packages/provider-java: the JAR ships in its own py3-none-any wheel,
    pulled in by exact-pinned `[java]` extras. Installing it is the consent
    for in-JVM instrumentation; without it nothing is injected and the
    diagnostic names the install. Discovered through the `platynui.providers`
    entry-point group — in-process where the runtime already runs inside
    Python, and through the co-located environment interpreter for the
    Inspector and CLI, which are `bindings = "bin"` and have no wrapper
    process. Embedding libpython was rejected: maturin ignores abi3 for
    binary targets, so those wheels would stop being version-independent.
  - The Java classification gains an agent-presence fact, reported by both
    the UIA and JAB providers, so "this window is empty" can become "this
    is a JVM, it has no agent, and here is the remedy". Reading it never
    triggers an attach.

  Two things the walking skeleton found that the design had assumed:

  - The `load` reply differs by JDK. Java 8 answers a bare integer, JDK 21
    answers `return code: 0`. The first parser rejected a *successful* JDK
    21 attach as a protocol error. Both dialects are parsed now, and an
    unrecognised body is treated as a refusal rather than as success.
  - JEP 451, measured on Temurin 21.0.11: `JAVA_TOOL_OPTIONS` genuinely
    controls `EnableDynamicAgentLoading` — proven by `-XX:-…` blocking the
    attach — so the sunset is remediable per environment without touching
    how an application starts, which is the load-bearing claim. Only the
    cosmetic warning suppression needs the flag on the real command line.
    A blocked load is reported as agent-refused, distinct from
    attach-failed, carrying the target's own remedy text.

  The Windows owner-only ACL was verified rather than assumed: a directory
  created under a parent granting Everyone full control still ends up with
  a single non-inherited owner ACE, and a deliberately loosened directory
  is re-hardened on the next start. Hardening therefore runs on every start.

  jattach is not vendored; the escape hatch stays unused. Two limits fail
  loudly instead of being worked around: x86-64 targets only, and a 32-bit
  JVM cannot be attached from a 64-bit process — both fall back to
  `-javaagent` with a named diagnostic.

  The Unix attach leg is implemented and cross-compiles clean for
  x86_64-unknown-linux-gnu, but has not run against a live JVM yet: no
  Linux session was available.

- **keyboard:** Harmonize key names across all platforms ([ec892b6](https://github.com/imbus/robotframework-PlatynUI/commit/ec892b67e43d18fe2953f89acb31f6fbe31e2ca9))

  Add missing aliases so every key name in the user guide works
  identically on Windows, Linux X11, and the mock platform.
  Fix Menu/Apps semantic mismatch and document Robot Framework
  double-backslash escaping.

- **keyboard:** Implement symbol aliases for reserved characters and enhance keyboard device functionality ([dee5900](https://github.com/imbus/robotframework-PlatynUI/commit/dee59001dc815d0ca227502d5e6ba3db1be0f5d2))
- **keyboard:** Add known_key_names method to KeyboardDevice trait and implementations ([bba29a1](https://github.com/imbus/robotframework-PlatynUI/commit/bba29a18f427ef6924feb148abde32762e9834ab))
- **keyboard:** Finalize mock stack and CLI ([92d69f0](https://github.com/imbus/robotframework-PlatynUI/commit/92d69f0a678e139a784a30a78b593bbf854f904e))

  - unify keyboard commands around sequence arguments

  - log key events in mock provider

  - update docs and tests

- **link:** Centralize provider linking; mock-only tests; docs update ([c5d3f3f](https://github.com/imbus/robotframework-PlatynUI/commit/c5d3f3f088ea9e6ca13f1da88a0b566ba1553cbc))
- **log-filter:** Build the log filter of every entry point in one place ([82d4ffb](https://github.com/imbus/robotframework-PlatynUI/commit/82d4ffb629c65ae2e3439fc4e6abdbc0e86bba48))

  platynui-log-filter builds the tracing filter for the command-line tool,
  the Inspector and the Python extension, which each had a copy of their
  own. RUST_LOG is used verbatim and is the only source of filter
  directives. Otherwise a single level, the one the entry point was given
  or PLATYNUI_LOG_LEVEL, becomes warn,platynui=<level> for warn to trace,
  and applies to every module for error and off. parse_level is case-
  insensitive, knows warning, critical and fatal, and serves as a clap
  value parser. A rejected value counts as absent and is reported once per
  process.

  The fmt feature installs the binaries' stderr subscriber; the log
  feature forwards log records into the same filter. The crate joins the
  cross-target package lists.

- **mock:** Render platynui testcard screenshot ([c980306](https://github.com/imbus/robotframework-PlatynUI/commit/c980306a2c433188f9339581ea84d7b4b1b51dbd))
- **mock:** Load mock tree from xml and improve query output ([87b83df](https://github.com/imbus/robotframework-PlatynUI/commit/87b83dfaffa2c793faf60dcb5990a5f4b33e4176))
- **native:** Use the shared log filter and report unusable input ([ec6524b](https://github.com/imbus/robotframework-PlatynUI/commit/ec6524b4aa2bda96c633daa62d7a69466bcdcbea))

  - The extension builds its filter with platynui-log-filter.
    set_log_level and native_log_level accept off, warning, critical and
    fatal, in any case.
  - Runtime(), Runtime.new_with_mock() and shutdown() deliver queued
    records before they return.
  - UiNode.describe() returns the element description; repr() shows the
    runtime id without asking the provider.
  - The configuration binding warns about a top-level key other than
    platform and providers, a bucket that is not a dict, and values it
    cannot pass on.
  - A profile value of the wrong type raises TypeError naming the key; an
    unknown profile key warns.
  - The keyboard calls take sensitive=True, which keeps the sequence out
    of the error.
  - A test build used without its mock backend warns once per process.

  Behavior changes: a single level no longer lowers third-party crates,
  RUST_LOG does; error and off apply to every module; PLATYNUI_LOG_LEVEL
  takes a single level only; a profile value of the wrong type, silently
  ignored before, raises TypeError.

- **native:** Forward native tracing to Python logging ([697fa58](https://github.com/imbus/robotframework-PlatynUI/commit/697fa58800feea73e7532522902c987f8234d92f))

  The extension installed no tracing subscriber, so every diagnostic the
  Rust core emits was dropped when PlatynUI ran from Python.

  At import the extension now installs a subscriber that turns each
  enabled event into a record and queues it. It never calls into Python
  from the thread that logged: native calls hold the GIL and some join
  threads that log, and Robot Framework drops messages from any thread
  but the one running the keyword. Queued records are delivered to
  Python logging on the calling thread when a runtime call returns (after
  the runtime lock is released, so a handler may call back into the
  runtime) and on flush_logs(). Each record goes to a logger named after
  the emitting module under platynui.native, with its level mapped one to
  one (trace becomes level 5, named TRACE), its fields in the message and
  in native_fields, and, for a record from another thread, that thread's
  name and the time it logged.

  Only warn and error are produced by default. set_log_level() lowers the
  floor for PlatynUI's own crates; RUST_LOG and PLATYNUI_LOG_LEVEL work
  with the command-line tool's precedence, and a value that does not parse
  is skipped and reported. The queue holds 10 000 records; overflow drops
  the newest and the next delivery reports how many were lost.

  A test emitter compiled into mock builds only covers delivery from a
  thread the call joins while holding the GIL.

- **native:** Value semantics for UiNode and EvaluatedAttribute ([76d0a28](https://github.com/imbus/robotframework-PlatynUI/commit/76d0a28eba6852d3b55a60282f3a66a391862ffc))
  - **BREAKING**: bool() of a UiNode and equality/str/hash of an
EvaluatedAttribute change behavior. No known consumer relied on the
previous always-True / always-False semantics.

  bool(UiNode) now reflects is_valid(), and EvaluatedAttribute delegates
  __bool__/__eq__/__ne__/__str__/__hash__ to its captured value. Evaluated
  query results therefore test truthy and compare like the value itself
  instead of being unconditionally True/False — the foundation the BareMetal
  wait keywords' truthy default builds on. Stubs and pytest cover the new
  behavior.

- **native:** Add typed get_pattern API and improve pattern handling ([648d9c0](https://github.com/imbus/robotframework-PlatynUI/commit/648d9c0bfcfc1c51785a9ec77f08550a2465346a))

  Replace pattern_by_id with unified get_pattern that accepts both a
  pattern class (type[T] -> T) and a string id. Both get_pattern and
  has_pattern now accept str | type via pattern_id_from_arg helper.

  get_pattern raises PatternError when the node does not support the
  requested pattern, replacing the previous silent None return. This
  makes unsupported patterns immediately visible to callers.

- **optimizer:** Add constant folding optimization and related benchmarks ([a2e5f43](https://github.com/imbus/robotframework-PlatynUI/commit/a2e5f43ada82ae98acaa51b83273e341760b0bc2))
- **patterns:** Wire up TextContent read-only text capability ([1c01872](https://github.com/imbus/robotframework-PlatynUI/commit/1c018722f216b9cc746d9bb18b6d276528e39e1d))

  Surface an element's current text as a canonical, read-only `control:Text`
  attribute on the real providers and make the `TextContent` client pattern
  resolvable on the Python side, so the Robot Framework `Text` widget can read
  its content.

  - core: add the `attribute_names::text_content::TEXT` constant; testkit uses
    it instead of the "Text" literal.
  - atspi: emit `control:Text` from the standard attribute iterator, gated on
    the AT-SPI Text interface, verbatim `GetText(0,-1)`, preserving empty
    strings (an empty field stays present-and-empty, not Null).
  - windows/uia: add `control:Text` from `TextPattern.DocumentRange.GetText`,
    falling back to `ValuePattern.Value`; never the accessible name; absent
    when neither pattern is supported.
  - python: synthesize `TextContent` from the `control:Text` attribute via the
    existing attribute-only convention (`_NativeTextContent`,
    `_build_textcontent`, `_ATTRIBUTE_ONLY_PATTERNS`).
  - trim the contract to just `text`: drop `locale`/`is_truncated` from
    `TextContent` and the now-orphaned overrides in the Edit/ComboBox proxies,
    test stubs, and tests.
  - egui test app + acceptance: seed the `input-name` field with known content
    and add real-AT-SPI coverage for present/absent `control:Text`.
  - docs: correct architecture.md 6.3/6.4 (drop IsReadOnly from TextContent;
    TextPattern-first UIA mapping; note read-only is a client-side derivation).

  Read-only is deliberately not a `TextContent` attribute; it is a client-side
  derivation (`TextContent ^ !TextEditable`) deferred to a future TextEditable
  change. Windows UIA is implemented and compile/clippy-checked from Linux;
  runtime verification runs on a Windows host.

- **platform:** Report window geometry and popup surfaces ([a4ca08b](https://github.com/imbus/robotframework-PlatynUI/commit/a4ca08bba4b87039c4474fd5b4aabfebaddf20e6))

  A window's visible frame and the origin of its surface are two facts:
  a toolkit that draws a client-side shadow places its surface outside
  the visible frame. WindowManager::bounds(id, toolkit_hint) becomes
  geometry(id), which returns both, so toolkit knowledge leaves the
  platforms. A new property says whether toolkit screen coordinates are
  desktop coordinates: true by default, false under Wayland. popups(pid)
  returns each popup's visible rectangle and its surface.

  Under the PlatynUI compositor every client-side-decorated window now
  gets its shadow inset, not only GTK 4 windows, and window_at_point
  reports the same visible frame. list_popups adds each popup's surface,
  which for a GTK 4 popover includes the shadow around it; the popover
  nodes of a nested submenu matched the listed surface sizes exactly.

  X11, Windows and the mock report their existing rectangles for now.
  The AT-SPI provider reads the visible bounds until its placement is
  reworked (linux-element-geometry group 5).

- **platform:** Add Linux Wayland platform crate and harmonize key names ([35e02c3](https://github.com/imbus/robotframework-PlatynUI/commit/35e02c3f532c2fe42659b6af09d5cb81279c3a24))

  Introduce the `platynui-platform-linux-wayland` crate, providing
  Wayland-native UI automation support on Linux via EIS (libei) for
  input injection and standard Wayland protocols for desktop info.

  Core capabilities:
  - Pointer input (absolute move, button press/release, scroll)
    via EIS PointerAbsolute/Button/Scroll interfaces
  - Keyboard input via EIS with layout-aware xkb-util reverse
    lookup and ~139 named key entries from input-event-codes
  - EIS session establishment with portal (Mutter/KWin) and
    direct LIBEI_SOCKET (PlatynUI compositor) connection paths
  - Desktop info via wl_output enumeration with multi-monitor
    union bounds
  - Compositor detection via SO_PEERCRED + /proc/pid/comm with
    env-var fallback (KWin, Mutter, PlatynUI compositor)
  - Session detection to distinguish Wayland-native vs XWayland
    apps for coordinate handling
  - Coordinate translation helpers for window-relative to screen
  - Stub infrastructure for screenshot (ext-image-copy-capture,
    wlr-screencopy, portal), highlight (layer-shell), and window
    management (foreign-toplevel, compositor IPC)

  Harmonize keyboard key names across all four platform crates
  (mock, X11, Windows, Wayland) so the same key name works
  everywhere:
  - Add long-form modifier aliases (LEFTSHIFT, RIGHTSHIFT, etc.)
  - Add META/LMETA/RMETA for the Super/Windows key
  - Add LMENU/RMENU as Alt aliases on all platforms
  - Fix MENU to map to the context-menu key (not Alt) on Windows
  - Add KP_* numpad aliases on Windows matching X11/Wayland naming
  - Add SYSRQ/SYSREQ, BREAK, HELP, Print aliases consistently
  - Make Point::new a const fn to support static coordinate values

- **platform:** Introduce cross-platform WindowManager ([ceb1645](https://github.com/imbus/robotframework-PlatynUI/commit/ceb1645595c44c1c974deca11310bd6d5c911f1a))

  Add a WindowManager trait in platynui-core that abstracts
  native window handle operations, decoupling accessibility providers
  from platform-specific windowing APIs.

- **platform-linux:** Check X11 and Wayland settings, explain the input ([24e9ae2](https://github.com/imbus/robotframework-PlatynUI/commit/24e9ae2b6d489b2b1b9d1ec3eee763b4c068fbf1))

  X11 reads display with a type check and warns about keys it does not
  know; Wayland warns about any key. The Wayland input setup records each
  rejected backend with its reason at debug and names the compositor with
  the chosen one; when none works, one warning names every backend with
  its reason.

  X11 records that name a key or keysym move from debug to trace, and a
  screenshot failure that is returned is logged at debug instead of error.

- **platform-linux:** Add session mediator crate for Linux platform dispatch ([d411d51](https://github.com/imbus/robotframework-PlatynUI/commit/d411d51b739168ae17b8c4b42fce48cf906e78df))

  Introduce platform-linux as the single Linux platform entry point
  that detects the display session (X11/Wayland) at startup and
  delegates all platform trait calls to the appropriate sub-platform.

  - Add session detection via XDG_SESSION_TYPE, WAYLAND_DISPLAY,
    and DISPLAY environment variables with process-lifetime caching
  - Resolve all platform backends once during initialization and
    cache as static trait-object references for zero-overhead dispatch
  - Wayland sessions temporarily fall back to X11 with a warning
    until native Wayland support is available
  - Convert platform-linux-x11 from a self-registering platform to
    a library crate consumed by the mediator (remove inventory
    registration, make modules and types public)
  - Update dependent crates (cli, inspector, native, playground) to
    depend on platform-linux instead of platform-linux-x11
  - Document the mediator architecture, session detection, and
    delegation design in architecture.md, platform-linux.md, and
    the project plan

- **platform-linux-wayland:** Add Wayland platform crate with real desktop info ([c69108d](https://github.com/imbus/robotframework-PlatynUI/commit/c69108dd2d51a233711d451dbd3e4c61a9d9ed86))

  Introduce the platform-linux-wayland crate that connects to the
  Wayland display server, detects the running compositor, and enumerates
  monitors with full resolution, position, scale, and transform data.

  - Create crate with all seven platform trait stubs (pointer, keyboard,
    screenshot, highlight, window manager) plus a real DesktopInfo
    implementation backed by wl_output and xdg-output-unstable-v1
  - Detect the compositor process (Mutter, KWin, Sway, etc.) via
    SO_PEERCRED on the Wayland socket for future backend selection
  - Enumerate outputs using GlobalList from registry_queue_init, with
    xdg-output logical coordinates preferred over wl_output geometry
    and a correct fallback that accounts for scale and transform
  - Wire the Wayland backend into the platform-linux mediator so it
    is selected automatically on Wayland sessions instead of falling
    back to X11
  - Update the Wayland compositor plan with Desktop Info completion
    status

- **platform-linux-x11:** Support displays without a window manager ([33339ae](https://github.com/imbus/robotframework-PlatynUI/commit/33339ae9befef3e3bea745760498886c806605f2))

  The X11 window manager now decides once per runtime whether an EWMH
  window manager runs. Without one it works with the X server's own
  window tree: an application's windows are its viewable,
  non-override-redirect root children; a window is active while it
  holds the input focus; activation maps, raises and focuses it while
  the process's popups stay above it; close sends WM_DELETE_WINDOW;
  minimize, maximize and restore fail by name. A new lane profile,
  real-x11-bare, runs the suites without a window manager, selected by
  window-manager:required and window-manager:none.

  Among several windows of one process the lookup now chooses by exact
  title, then by a size only one has, then by the screen position a
  toolkit reports, and fails naming the count otherwise. GTK 4 reports
  no screen positions, so geometry no longer comes first.

  The window geometry leaves out GTK's client-side shadow
  (_GTK_FRAME_EXTENTS) from the visible bounds, and the backend lists a
  process's popup windows with their visible rectangle and surface.
  Without a window manager toolkit screen coordinates are not desktop
  coordinates: egui kept the old origin after another client moved its
  window.

- **platform-linux-x11:** Implement X11 keyboard device ([72d7d73](https://github.com/imbus/robotframework-PlatynUI/commit/72d7d731bb5f464b46fe5b223f0152d62927a7db))

  XTest-based keyboard injection with keysym-to-keycode resolution
  via GetKeyboardMapping. Supports named keys (~120 entries),
  single-character input with CapsLock-aware shift management, and
  dynamic keycode remapping for characters outside the active layout.

  Control characters (\n, \t, \r, etc.) are mapped to their
  corresponding X11 TTY function keysyms (Return, Tab, etc.).

- **platform-linux-x11:** PlatformModule initialization with eager connection and extension checks ([fd3ffca](https://github.com/imbus/robotframework-PlatynUI/commit/fd3ffcac853fb5cd16fbfaed24a14e2d43812fa2))
- **platform-mock:** Check its settings and trace per-key records ([8ff275d](https://github.com/imbus/robotframework-PlatynUI/commit/8ff275d2caf7bf86269f9d9a8ca90ecd9c3beacd))

  The mock platform warns about keys in its section, none of which it
  reads. The mock keyboard's press and release records and the mock
  pointer's move record move from debug to trace: a record that names a
  key is trace-level, because the keyboard cannot tell a Secret from a
  plain string, and one record per move is per-item output.

- **platform-wayland:** Add input injection for Wayland compositors ([0daf8f3](https://github.com/imbus/robotframework-PlatynUI/commit/0daf8f30e4e7867fcac0f3b479f7dc25e85ce7e6))

  Implement three input backends in the Wayland platform crate that enable
  keyboard and pointer control on GNOME, KDE, and wlroots-based compositors:

  - EIS (libei via reis): for Mutter, KWin, wlroots — connects via direct
    EIS socket or XDG RemoteDesktop Portal with token persistence
  - VirtualInput: zwlr-virtual-pointer/keyboard with local xkb::State
    for explicit modifier tracking (fixes <Control+A> et al.)
  - ControlSocket: fire-and-forget JSON IPC for the PlatynUI compositor

  CompositorType-based backend selection with automatic fallback ensures
  the best available input path is used on each compositor. The InputBackend
  trait unifies all backends behind WaylandKeyboardDevice and
  WaylandPointerDevice.

- **platform-wayland:** Implement desktop info with live monitor updates ([31c5f70](https://github.com/imbus/robotframework-PlatynUI/commit/31c5f705277b2ae6112b504ccfc0b78387f237df))

  Provide full desktop and monitor information on Wayland through
  wl_output, xdg-output-manager, and compositor-specific D-Bus APIs,
  with continuous runtime updates for hardware and configuration
  changes.

  - Enumerate monitors via two-phase Wayland roundtrip collecting
    geometry, mode, scale, name, and logical layout from xdg-output
  - Enrich outputs with exact fractional scaling and primary monitor
    flag from Mutter GetCurrentState() and KWin primaryOutputName
  - Compute physical pixel coordinates from the compositor's logical
    layout, propagating native dimensions along adjacent edges
  - Show compositor name in desktop title: Wayland Desktop (Mutter)
  - Run a background dispatch thread that monitors the Wayland socket
    for output hot-plug/unplug, resolution, scaling, and layout
    changes, automatically re-enriching and updating desktop state
  - Own output data model and storage in the desktop module,
    keeping the connection layer focused on Wayland protocol handling

- **platform-windows:** Check its settings, log returned errors at debug ([01b3370](https://github.com/imbus/robotframework-PlatynUI/commit/01b3370651bbf3ea0c8a32812ecbaaa54648bde0))

  The Windows platform warns about keys in its section, none of which it
  reads. SendInput and screenshot failures, which are returned, are logged
  at debug; SendInput's error is read before anything is logged, so that
  no subscriber can overwrite it.

- **platform-windows:** Implement Win32 WindowManager ([4a6f730](https://github.com/imbus/robotframework-PlatynUI/commit/4a6f7305982a851cb257a44182636ad0f8a6b6cc))

  Provide a working WindowManager for Windows using native Win32 APIs.
  Supports window resolution via NativeWindowHandle and PID fallback,
  bounds queries, minimize/maximize/restore/close, move and resize.

- **platform-windows:** Implement keyboard device ([5e60335](https://github.com/imbus/robotframework-PlatynUI/commit/5e60335c4aed54f3eed3256c404ec1708f41863f))
- **platynui:** Add take_screenshot keyword to BareMetal Library ([74c8ea8](https://github.com/imbus/robotframework-PlatynUI/commit/74c8ea80187e477987d5921931a43e366b1eab72))
- **platynui:** Add a lot of keywords for BareMetal library ([0fc1fba](https://github.com/imbus/robotframework-PlatynUI/commit/0fc1fba9c475837f77399b2025fe7aa0342e5447))
- **pointer:** Improve pointer motion styles and add movement documentation ([47eeec2](https://github.com/imbus/robotframework-PlatynUI/commit/47eeec213a7d405d95c372e77512fa642657e905))

  - Pointer motion styles (Bezier, Overshoot, Jitter) now look noticeably
    different from straight-line movement — previously the effect was too
    subtle to see
  - Jitter motion now produces realistic wobble along the path instead of
    a single slight bump
  - New jitter frequency setting controls how many wobbles occur during
    a Jitter movement
  - All motion shape settings can now be fine-tuned via CLI flags such as
    --curve-amplitude, --jitter-amplitude, and --jitter-frequency
  - New documentation page (docs/pointer-input.md) explains how pointer
    movement, clicking, scrolling, and dragging work, including all
    available motion styles and tuning options

- **pointer:** Add multi-click functionality and related error handling ([e7b08bf](https://github.com/imbus/robotframework-PlatynUI/commit/e7b08bf5b10490691672dc1f61557a8f3e2610ad))
- **pointer:** Activate profile timing controls ([26e67c1](https://github.com/imbus/robotframework-PlatynUI/commit/26e67c1f8f7175bddeb82a98f31f8812c436b755))
- **pointer:** Add configurable move timing ([20ca97b](https://github.com/imbus/robotframework-PlatynUI/commit/20ca97ba7ac0a1f48536c2d259574ba230a893a1))
- **pointer:** Add move duration and time per pixel options for pointer movement ([1dc3003](https://github.com/imbus/robotframework-PlatynUI/commit/1dc3003127aafe26897b36e275e7d29a0c4a6c9f))
- **pointer:** Expose pointer API and CLI command ([a9b8be9](https://github.com/imbus/robotframework-PlatynUI/commit/a9b8be901d3c039f6f59380ad423d701c8aa659d))
- **process:** Read process attributes through the recorded identity ([3c151f1](https://github.com/imbus/robotframework-PlatynUI/commit/3c151f18e1f889e53fa30abea3799ad977eb3b75))

  Every provider read the process table on its own, and they disagreed on
  formats, on placeholders and even on which process they described. One
  reader per platform, bound to the identity an application node records,
  gives every provider the same answer, and never the answer of a process
  that received a reused pid.

  Windows reads natively through one limited-rights handle. Linux reads
  through sysinfo, except the command line, whose arguments sysinfo trims.

- **process:** Identify a process by its pid and start time ([8f6fc02](https://github.com/imbus/robotframework-PlatynUI/commit/8f6fc02c0f4352ae177f2e6d3aa306f0992272b9))

  Application nodes need to know whether the process they stand for
  still runs, and a pid alone cannot tell once the system hands it to a
  new process. The crate records the start time with the pid and answers
  running, ended or cannot tell: a process that cannot be inspected is no
  proof that it ended.

  It is a crate of its own because UI Automation, the Access Bridge and
  the Java agent all need it, and application-process-attributes adds
  its process readers to it later.

- **provide-windows-uia:** Enhance virtualized item handling and improve parent-child relationships ([d56105d](https://github.com/imbus/robotframework-PlatynUI/commit/d56105de2f92cba71a031aeeaadf88b7ddbc6406))
- **provider-atspi:** Place elements relative to their window ([3702847](https://github.com/imbus/robotframework-PlatynUI/commit/3702847f9707b4acf84935e451ba4766e27c328e))

  The AT-SPI provider placed a node by adding its parent-relative
  extents to its parent's bounds, up to the window. GTK 3, Qt before 6.5
  and Firefox answer parent-relative coordinates with screen ones, so
  the sum counted every ancestor again, and egui 0.37's containers
  without extents ended the chain: every egui click under the PlatynUI
  compositor missed.

  A node is now placed from its reference window, the nearest ancestor
  that is a top-level window or a popup the platform lists: the
  window's origin plus the node's position relative to it. GTK 4 is
  read in window coordinates, every other toolkit in screen coordinates
  unless a node reports no screen position of its own. Screen
  coordinates stand where the platform says they are desktop
  coordinates and are re-based on the window elsewhere. GTK 4 counts
  from the visible frame, other toolkits from the surface origin.

  Popups are matched to the platform's popup list by the size of their
  surface or visible rectangle: GTK 4 popovers by their surface, GTK 3
  menus by their visible size, and GTK 3 hands a menu's reference from
  the item that opened it to the items listed beside the menu. The
  hit-test prefers a hit inside a popup, and one in a submenu over the
  menu that opened it. UiNode gains as_any so the provider reaches the
  parents it built.

  The GTK 3 pointer tests and the GTK 4 popover tests from
  add-gtk-test-app join the suites, with a geometry suite per GTK
  fixture; both fixtures report a radio selection on the last-action
  label. A label inside a padded GTK 4 button reports its parent's
  origin, which stays a documented toolkit limitation.

- **provider-atspi:** Warn once per application, not per call ([a33896d](https://github.com/imbus/robotframework-PlatynUI/commit/a33896df68b869d4136d3ee5f2a809619bad992f))

  A call into an application that times out warns once per application
  instance, naming the application, its pid and bus name, the call and the
  timeout. Further timeouts are debug, and the episode ends when the
  application leaves the registry. Timeouts the provider returns, the
  connection failures and a slow application lookup are debug, so that a
  failure is not reported twice. Bounds substitutions use the transition
  latch and the element description, and the provider warns about mistyped
  and unknown settings.

- **provider-atspi:** Classify menu entries in the control namespace ([665d05e](https://github.com/imbus/robotframework-PlatynUI/commit/665d05ecc0a46597dbb6bc5d26f51c8adf014758))
  - **BREAKING**: On AT-SPI (Linux), menu entries no longer match

  Menu entries are invokable controls (activatable like a button, may own
  a submenu), not data items of a collection container, so the item
  namespace never fit them. Move MenuItem, CheckMenuItem, RadioMenuItem
  (all emitted as MenuItem), and TearoffMenuItem from the item to the
  control namespace in the AT-SPI role table. This matches the item
  namespace's own definition (ListItem, TreeItem, ...), the Windows UIA
  provider (IsControlElement already yields control there), and the
  Python model (MenuItem subclasses Control), making menu-entry locators
  portable across providers.

  Menu, MenuBar, and PopupMenu were already control; collection data-item
  roles stay in item. The BareMetal namespace legend no longer lists
  MenuItem under item:. The Qt acceptance locators drop the now-obsolete
  *:MenuItem namespace wildcard, and a new menu.robot suite verifies
  against a real provider that //MenuItem resolves an open menu entry,
  //item:MenuItem does not, and activation works (File-menu actions in
  the Qt test app rename themselves to <ident>-activated on trigger to
  make activation observable by locator). Verified on the headless
  compositor acceptance lane: 51/51 pass.

- **provider-atspi:** Surface transient popups via AT-SPI events ([2a8e662](https://github.com/imbus/robotframework-PlatynUI/commit/2a8e662ea5494688a1a8dc91cd40e994837eb114))

  Toolkits expose transient popups (right-click context menus)
  asymmetrically: the popup accessible walks back to an in-tree owner via
  parent(), but the owner's own GetChildren never lists it, so the
  top-down tree walk missed it entirely — context menus were invisible to
  XPath, the Inspector, and the hit-test (verified on Qt and GTK4 with
  the atspi_focus_watch example).

  A background worker on a dedicated bus connection (blocking calls on
  the stream's own connection deadlock it) registers for
  object:state-changed and object:children-changed, records shown popups
  with their owner in a registry, and tree enumeration grafts them under
  that owner. The hit-test searches grafted popups before the WM-scoped
  frame search, since the WM reports the managed frame beneath an
  override-redirect popup. A reachability check at registration keeps
  in-tree popups and real windows out of the registry.

  Grafted popups are no longer treated as window surfaces: resolving them
  through the window manager PID-matched the app's managed main window,
  which poisoned the whole subtree's bounds and let auto-activation
  dismiss the menu mid-interaction. Popup-class roles now resolve their
  geometry via AT-SPI screen extents and expose no window patterns.

  Rollback flag: providers.atspi.surface_popups (default on).

  The Qt test app's context menu gained a submenu cascade, and the new
  context_menu.robot acceptance suite covers open-menu findability by
  query and hit-test, submenu and nested-submenu resolution, and
  dismissal across all cascade levels. Pointer clicks into popups need
  compositor-side popup positions on Wayland, so those tests skip there
  (follow-up).

- **provider-atspi:** Add process metadata attributes for Application nodes ([586c0d2](https://github.com/imbus/robotframework-PlatynUI/commit/586c0d20c50750915a69482d85c6b15289ca9466))

  Implement six app:* attributes on AT-SPI2 Application nodes for parity
  with the Windows UIA provider:

  - app:ProcessName   — executable stem from /proc/PID/exe
  - app:ExecutablePath — full path via readlink /proc/PID/exe
  - app:CommandLine   — /proc/PID/cmdline (NUL → space-joined)
  - app:UserName      — effective UID from /proc/PID/status → /etc/passwd
  - app:StartTime     — /proc/PID/stat field 22 → ISO 8601 UTC
  - app:Architecture  — ELF e_machine from /proc/PID/exe

  Adds crates/provider-atspi/src/process.rs with pure-Rust /proc helpers
  (no libc dependency). Integrates new attribute structs into AttrsIter
  for Application nodes (guarded by process_id.is_some()).

  Updates docs/planning.md: marks AT-SPI2 application attribute tasks as
  complete, fixes stale app:Name → app:ProcessName references.

- **provider-atspi:** Filter out own process from accessibility tree ([3602463](https://github.com/imbus/robotframework-PlatynUI/commit/3602463b4f4c968e91a476f996ca5b2a5ba46e66))
- **provider-atspi:** Expose CoordType variants for Component/Image extents and position ([a37d205](https://github.com/imbus/robotframework-PlatynUI/commit/a37d2059d7242653aa5810dab7eb8ed6b5af8c3b))

  Replace the single hardcoded Screen coordinate type with explicit
  Screen/Window/Parent variants for spatial native attributes:

  - Component.Extents.{Screen,Window,Parent}
  - Component.Position.{Screen,Window,Parent}
  - Image.Extents.{Screen,Window,Parent}
  - Image.Position.{Screen,Window,Parent}

  The suffixless Extents/Position attributes are removed since the
  standard Bounds/ActivationPoint attributes already cover the Screen
  coordinate case through the LazyNodeData path.

- **provider-jab:** Publish the bridge's description as Description ([fa6b54f](https://github.com/imbus/robotframework-PlatynUI/commit/fa6b54f57cd47d70ed5473ab82b7d99c8984b7a3))

  The bridge reports a description, but the JAB backend listed it only
  as native:Description, so the same Swing application had @Description
  and a description accessor through the agent and not through the
  bridge. It is now listed as control:Description when it is not empty,
  and the accessor returns it.

- **provider-jab:** Project interface data as native attributes ([5334e21](https://github.com/imbus/robotframework-PlatynUI/commit/5334e21b9458252841009cd7b13759ac020dd646))

  Expose the data behind each supported JAB accessibility interface as
  native:<Interface>.<Property> attributes (jab-interface-attributes),
  the Swing analogue of UIA collect_native_properties:

  - new FFI structs/bindings and typed JabClient methods for the table,
    text-selection, action, hypertext, key-binding, and relation getters
    (embedded JOBJECT64 handles released on the pump thread)
  - single property catalog (interfaces.rs), bitfield-gated, with
    JVM-free well-formedness tests; container tier listed per node,
    per-cell TableCell.* listed only on table children (free parent-role
    gate) with every value resolved lazily per read
  - discovered JDK quirk: the Windows bridge resolves JTable cell
    lookups to the one shared cell renderer (volatile names, no bounds,
    no isSameObject identity); cell coordinates therefore derive from
    the tree parent context plus the enumeration index, and cells are
    addressed positionally
  - Swing fixture gains TablePanel (4x3 JTable, row 2 preselected) plus
    the native_attributes acceptance suite; hygiene and live-fixture
    guards extended over the table; JTable limitations and the solution
    space documented in dev-docs/platform-windows.md

- **provider-jab:** Point-based hit-testing via element_at_point ([f011705](https://github.com/imbus/robotframework-PlatynUI/commit/f0117058273448d98be19f2bdd8afd119a4c95b8))

  Implement OpenSpec change add-jab-hit-test: the Inspector's live picker
  can now resolve the real Swing control under the cursor instead of the
  empty UIA shell.

  - Bind getAccessibleContextAt / getAccessibleParentFromContext and add
    JabClient::context_at / parent (pump thread, per-call deadline, RAII
    handle release).
  - JabProvider::element_at_point: WindowFromPoint -> GA_ROOT, own-process
    skip, isJavaWindow gate (non-Java points report UnsupportedOperation),
    native hit-test for the deepest context.
  - Geometric fallback: the JDK's native hit-test answers null for every
    point until the target JVM has seen a mouse event
    (EventQueueMonitor.currentMousePosition), so descend over calibrated
    child bounds when the bridge reports no context; a point outside every
    child resolves to the window node.
  - Reveal-ready result: app-scoped node with a strong parent chain up to
    app:Application whose RuntimeIds match top-down traversal (bounded
    re-walk matched with isSameObject; parentless window-scoped fallback).
  - provider-windows-uia: element_at_point abstains for windows claimed by
    another provider (honor_window_claims kill switch, unit-tested), so a
    claimed Java window resolves to its JAB node independent of provider
    registration order.
  - Tests: Swing picker acceptance suite (identity, RuntimeId equality,
    app:Application ancestor); hit-test-during-freeze assertions in the
    frozen-JVM live lane.
  - Docs: JAB hit-test + UIA claim-abstain notes in platform-windows.md;
    sync the jab-hit-test delta spec into openspec/specs.

- **provider-jab:** Add Java Access Bridge provider for Windows Swing/AWT ([6233a43](https://github.com/imbus/robotframework-PlatynUI/commit/6233a43f04ae80c905139c9cf76c9e2731df467e))

  Java Swing/AWT apps implement no UIA provider, so their windows are
  opaque to the Windows UIA provider. The new Windows-only crate
  platynui-provider-jab reads their accessibility trees through the JDK's
  own out-of-process channel — the Java Access Bridge — the same one
  screen readers use. Nothing is loaded into the target JVM beyond the
  JDK's own bridge, and the provider never mutates target-side config.

  - Private JAB client layer: FFI over WindowsAccessBridge-64.dll with
    compile-time struct-layout asserts, DLL discovery, a dedicated
    message-pump thread owning all bridge calls with per-call deadlines
    and degraded-vmID tracking, RAII handle release, and a typed client.
  - Lazy JabNode: roles normalized from role_en_US to the PlatynUI
    vocabulary, context info read live per access, self-calibrating
    per-window DPI transform, RuntimeId jab://<vmID>/<hwnd>[/<path>]
    scoped per view, app:Application grouping with process metadata.
  - Patterns: Focusable, ActivationTarget, TextContent, TextEditable,
    Toggleable, StatefulValue, Selectable/SelectionProvider, Expandable;
    window patterns delegate to the injected WindowManager (top-level
    bounds come from it, since JAB frame bounds lag out-of-band moves).
  - Discovery via EnumWindows + isJavaWindow, single-appearance via the
    core window-claims registry, warn-once "bridge not enabled"
    diagnostics (never configuration mutation, pinned by a unit test).
  - event_capabilities: None (runtime polls); JAB callbacks are a
    future push enhancement, not required for correct reads.

  Wires the crate into the Windows link set (platynui-link plus the cli,
  native, inspector, and playground consumers), adds the Swing fixture
  build/run and JAB-live steps to the acceptance recipe (justfile), and
  ships the Windows Swing acceptance suite (tests/acceptance/swing) plus
  an #[ignore]-gated live-fixture Rust lane serialized via the nextest
  jab-live test group.

- **provider-java:** Report a missing bridge DLL only when it matters ([ab496bc](https://github.com/imbus/robotframework-PlatynUI/commit/ab496bc4cb42d732870d0b8c04c24d5e39fd8b3e))

  A missing Java Access Bridge DLL is debug at startup. Without it, the
  JAB backend still lists the Swing and AWT windows it sees as unserved,
  with their processes, so that automatic agent attachment can reach them.
  When such a window stays unserved, the router warns once per process,
  naming providers.java.jab.dll_path, PLATYNUI_JAB_DLL and a 64-bit JDK.

  The agent's version mismatch is warned once per JVM. Only timeouts and
  transport, protocol and no-agent errors mark a JVM degraded; an answer
  with an error proves the agent is alive. The provider and both backends
  warn about mistyped and unknown settings, the backends' as agent.<key>
  and jab.<key>.

- **provider-java:** Serve agent-backed tables as rows of cells ([1ef9314](https://github.com/imbus/robotframework-PlatynUI/commit/1ef931477822ccd410aa94bdcfef54d00081362a))

  A Swing table served by the in-JVM agent was one flat, row-major list of
  cells -- the shape `AccessibleContext.getAccessibleChild(i)` offers and all
  the Access Bridge ever had, inherited rather than chosen. The agent reads
  the toolkit's own model, where a row is a first-class thing, so it now
  reports `Table` -> `item:TableRow` -> `item:TableCell`, which is the shape
  `provider-atspi` and `provider-windows-uia` already surface for tabular
  content. Cells keep everything they had, one level deeper.

  A table's shape therefore follows the backend that serves it: nested
  through the agent, flat through the bridge. That cost is deliberate and
  observable -- `@Technology` says which one answered -- and it is the
  better shape on the path most runs take.

  Two consequences the accessibility view forces:

  - `AccessibleSelection` on a `JTable` answers in CELL indices, which no
    longer address a direct child, so `SwingTree.childAt` declines for a
    table rather than translating, and the selection is re-derived from
    `getSelectedRows()`. `control:SelectedItems` names the selected rows;
    cell-level selection stays on the cell.
  - Hit-testing gains a level: `chainAt` appends the interned row and then
    the interned cell, both the same objects the enumeration hands out.

  The fixture table grows to 100x6 in a viewport it does not fit, and that
  exposed a real defect: `getCellRect` answers from the model regardless of
  scrolling, so row 90 published bounds at y=2808 -- some 2000px below the
  window -- while claiming to be in view. Pointer input aims at a
  rectangle's centre, so that was an instruction to click another
  application. Geometry is now intersected with the table's visible rect:
  off-view content reports no bounds, no activation point and
  `IsInView = false`, and partially scrolled content is clipped so its
  centre stays where the user can see it.

  The volume also surfaced a latent flake: `hygiene.robot` compared cell
  names across walks, which the bridge's renderer aliasing makes volatile --
  the suite's own documentation already said so, and with twelve cells it
  never fired. The node count still covers every cell.

  Verified on Windows: `just check`, `just test` (2152), `just
  test-java-agent`, the live fixture lane (10/10) and the Swing acceptance
  lane (36/36, against the provisioned Java 8).

- **provider-java:** Serve Java trees through the in-JVM agent ([eba0409](https://github.com/imbus/robotframework-PlatynUI/commit/eba04098cf7f63068b83c365cf058c7d6a517349))

  Adds the agent backend and the mapping layer that turns the agent's
  element model into PlatynUI nodes, and reshapes the backend surface so
  the router can choose between two channels for the same window.

  Selection is the backend *order* and nothing else. The backends sit in
  preference order — agent, then Access Bridge — and the router records
  what each served before the next runs, so a weaker backend already sees
  the windows it must leave alone within the same pass. Ownership is
  preference-aware rather than first-come, or an agent appearing in a JVM
  the bridge is serving could never take the window over.

  Selection had to become the backend's *input*, not a filter on its
  output: an `app:Application` node enumerates its windows lazily, after
  the pass returned, so a post-filter provably cannot reach them. Hence
  `WindowExclusions` in the JAB crate and `BackendOwnership`/
  `ForeignWindows` in the router.

  Automatic attachment is on by default and takes effect in the pass that
  triggered it: the router waits, bounded, for the injected agent's
  handshake and then sweeps again, discarding the first sweep rather than
  editing it — which node stands for which window is exactly what a flat
  node list cannot answer. Without this a caller saw the bridge until it
  enumerated again; in the Inspector, two refreshes.

  The trigger is "this JVM carries no agent", not "no backend can serve
  this window": a bridge-served window still has a better representation
  available, and gating on unserved windows would have limited attachment
  to JVMs that were already broken.

  Notable details:

  - RuntimeIds are identity-based (`agent/<pid>/<element-id>`), so a
    relayout that reorders siblings renames nothing.
  - `is_valid` is answered from the agent's registry and answers **false**
    when the agent is degraded or unreachable — an optimistic `true` pins
    a dead scoped root forever.
  - A call abandoned at its deadline leaves the stream desynchronised: the
    agent may still write that answer later. The session therefore drops a
    timed-out connection and rebuilds it, or one wedged moment cost that
    JVM the rest of the run.
  - The window handle falls back to PID **plus geometry** against
    EnumWindows, and refuses an ambiguous match — the platform layer's
    PID-only path documents that it "can target the wrong sibling window".
  - `@Technology` stays the channel (`JavaAgent` vs `JAB`); the toolkit is
    `native:JvmToolkit`, on the names every other provider already uses.

- **provider-macos-ax:** Say once that macOS support is missing ([e7682da](https://github.com/imbus/robotframework-PlatynUI/commit/e7682dadbcc0e25c2c4b07b7be4241fe0c0edbbc))

  The stub provider warns once per process that queries on this desktop
  find nothing because macOS support is not implemented yet, and warns
  about any key in its section.

- **provider-mock:** Warn about settings it does not read ([492f2a7](https://github.com/imbus/robotframework-PlatynUI/commit/492f2a70951e8a037518f4b7e15dcff68bc16a0f))

  The mock provider checks its section at build time and warns about each
  key, since it reads none, as every PlatynUI component now does for its
  own settings.

- **provider-mock:** Make mock activate() exclusive ([14a597d](https://github.com/imbus/robotframework-PlatynUI/commit/14a597dd46df3331f022566a676aa32931f902ff))

  A real window manager keeps exactly one active window. Mirror that in the
  mock: activating a window now clears is_active/is_topmost on every other
  window before setting them on the target. This makes @IsActive a reliable
  single-foreground discriminator (which the mock-lane auto_activate tests
  rely on) and better reflects real provider behaviour. node_updated is
  emitted for each deactivated window as well as the activated one.

- **provider-uia:** Add support for window state attributes (IsMinimized, IsMaximized, IsTopmost) and user input acceptance ([b9a45ef](https://github.com/imbus/robotframework-PlatynUI/commit/b9a45ef43296f25792e6dc65fe04ec0528f01d3d))
- **provider-window-uia:** Implement WaitForInputIdleChecker ([6927795](https://github.com/imbus/robotframework-PlatynUI/commit/6927795b54aaee0aec2b784bd07b0bf7e3966790))
- **provider-windows-uia:** Check its settings, trace window activation ([6983c64](https://github.com/imbus/robotframework-PlatynUI/commit/6983c644530de4eb75addf372bcc9c26d25d8330))

  UI Automation warns about unknown keys and a mistyped
  honor_window_claims. Window activation records at debug when it starts
  and when the window did not become the foreground window.

- **provider-windows-uia:** Skip windows claimed by another provider ([0e5f497](https://github.com/imbus/robotframework-PlatynUI/commit/0e5f4977296cfb395ed2454b86a515662c3c13e5))

  During root streaming, consult the process-wide window-claims registry
  and skip top-level windows another provider has claimed (config
  providers.windows-uia.honor_window_claims, default true), so a Java
  window represented by the JAB provider appears exactly once in the
  merged desktop tree. With the kill switch off, the UIA shell reappears
  alongside the JAB node, distinguishable via @Technology.

- **provider-windows-uia:** Implement scoped RuntimeId URIs for UiaNode and related attributes ([c7c4623](https://github.com/imbus/robotframework-PlatynUI/commit/c7c462327b3084199a5c21f5645cda04c3716a5c))
- **provider-windows-uia:** Implement Application view ([c51a3f4](https://github.com/imbus/robotframework-PlatynUI/commit/c51a3f459b12e9082ed8f11df3fc171ea71a93ed))
- **provider-windows-uia:** Implement native property support ([b262755](https://github.com/imbus/robotframework-PlatynUI/commit/b26275595166ffcc1a78e62245ba2dafea48a99d))
- **python:** Bind runtime config dict via Runtime(config) and BareMetal config= ([25ad716](https://github.com/imbus/robotframework-PlatynUI/commit/25ad716c133813d40d938dbdf6a196987e37a2e0))

  Exposes the construction-time session config on the Python surface (change
  per-runtime-platform-sessions §7).

  - packages/native: `Runtime(config=None)` parses a nested dict into
    `RuntimeConfig` -- two id-keyed buckets (`platform` / `providers`), recursive
    PyDict->ConfigMap, `bool` checked before `int`, unsupported leaves / non-string
    keys / non-dict sections skipped with a debug log (never an error). `None` or
    empty ยง= `RuntimeConfig::default()` (unchanged behavior). `.pyi` updated.
  - BareMetal: new construction-time, immutable `config=` argument forwarded to
    `Runtime(config)`, documented in the docstring. `use_mock` keeps routing to
    `new_with_mock()` (the mock provider is not inventory-discovered, so a
    backend=mock config would still load real AT-SPI) -- behaviorally identical.
  - native pytest covering mock-backend selection, empty no-op, and tolerance of
    unknown/foreign/mixed-type entries.

- **python:** Resolve @locator method form, drop ApplicationReady ([e2ad1d3](https://github.com/imbus/robotframework-PlatynUI/commit/e2ad1d3aabf0489289f62b295e7ba64520e5ea69))

  Close the remaining Phase 3 and Phase 4 loose ends.

  @locator method/property form now resolves: LocatorMethodDescriptor.__get__ (bare form) and __call__ (@property form) read the wrapped function's return-type annotation (a ContextBase subclass) and call instance.get(annotation, locator=...). A non-ContextBase annotation or owner raises a clear TypeError. Closes the last Phase 3 stub.

  Drop the orphan ApplicationReady pattern instead of wiring it: a spike confirmed it would only ever delegate to the same responsiveness probe as Responsive on every real backend, i.e. a pure duplicate. _application_is_ready keeps using Responsive (native) plus Application.is_ready() (user hook); the ABC and its export are removed.

  Record the decision in the plan (status Rev. 49, design A.6/A.13.2) and add the future Wait Until Ready keyword to the Phase 5 catalog.

- **python:** Add support for mock runtime in BareMetal library initialization ([0a2f425](https://github.com/imbus/robotframework-PlatynUI/commit/0a2f42510ed1381e6cd16681ec574f6c2046e095))
- **python:** Stabilize python modules ([92aaf94](https://github.com/imbus/robotframework-PlatynUI/commit/92aaf9499a1ff9dfbb307a82d9c2db4b8e234262))
- **python:** Add first Python bindings for core and runtime ([2111984](https://github.com/imbus/robotframework-PlatynUI/commit/2111984a1741ae1306bc4f7ad0275c0e53617994))
- **python-baremetal:** Add highlight keyword and improve type annotations ([59df846](https://github.com/imbus/robotframework-PlatynUI/commit/59df846baa9c3b99b9f9b9e05db7d08af4a6086b))
- **python-bindings:** Add highlight/clear_highlight and screenshot to runtime ([7d64e50](https://github.com/imbus/robotframework-PlatynUI/commit/7d64e50331bcb983c262627830c9d0f248815e4e))
- **python-bindings:** Expose desktop_node/desktop_info/focus in runtime module ([0441dde](https://github.com/imbus/robotframework-PlatynUI/commit/0441dde0c46609f060b621d9491f56ff7ff23506))

  - Add Runtime.desktop_node(), desktop_info() dict conversion, focus(node)
  - Update typing stubs and concept doc
  - Add Python tests for desktop info and basic focus

- **python-bindings:** Make UiAttribute.value() lazy method instead of property ([c068fbf](https://github.com/imbus/robotframework-PlatynUI/commit/c068fbfc6c7885d719aa0d0d9aa1ce59c4a92f0e))

  - UiAttribute now holds owner node and resolves value on demand
  - runtime.pyi updated: value() returns UiValue | None, no property
  - Concept doc updated to reflect lazy access

- **python-native:** Add docs to python interfaces ([14f4606](https://github.com/imbus/robotframework-PlatynUI/commit/14f4606d4c29242e30d3014c36ea30a122882967))
- **python-native:** Enhance PointerOverrides with additional motion parameters and update related methods ([ee33bc3](https://github.com/imbus/robotframework-PlatynUI/commit/ee33bc3ecf26eb1f4f6691c83bb4dad8f3479322))
- **python-native:** Enhance pointer and keyboard settings with new type annotations and structures ([b81a059](https://github.com/imbus/robotframework-PlatynUI/commit/b81a0592c225b0334f6864d05589531feb09965e))
- **python-native:** Add pointer and keyboard settings API ([107cbab](https://github.com/imbus/robotframework-PlatynUI/commit/107cbab3ec5eb782992b5e6ddd8b477e37398afe))
- **python-native:** Enhance Point, Size, and Rect classes with 'from_like' methods and support for tuple/dict inputs ([77c7161](https://github.com/imbus/robotframework-PlatynUI/commit/77c71619de54c7e466b21a74a7a1bf16ffad8dd2))
- **python-native:** Add AttributeNotFoundError and update exception hierarchy ([c2f91d1](https://github.com/imbus/robotframework-PlatynUI/commit/c2f91d13c9d6a1394505fa7aa16cb628b71deb31))
- **python-native:** Add methods for ancestor traversal and pattern retrieval ([654bd61](https://github.com/imbus/robotframework-PlatynUI/commit/654bd61b44c0f0fcf6a120cff529430d6ebcce7f))
- **python-native:** Add is_valid method to UiNode for liveness checks ([a49e26c](https://github.com/imbus/robotframework-PlatynUI/commit/a49e26cf17c0ce07392f9a5a2c7166c77910bbcb))
- **python-runtime-native:** Simplify python package/module structure and rewrote pyi files ([5efc4ff](https://github.com/imbus/robotframework-PlatynUI/commit/5efc4ffb3170a7dba699fe6fbfb834825b2d7f94))
- **repl:** Implement XPath REPL example with error handling and sample document ([b1caf5d](https://github.com/imbus/robotframework-PlatynUI/commit/b1caf5d029dd1b90e9a560947dce1a95984383c8))
- **runtime:** Name decisions, report once, and type without echoing ([e1bd6b7](https://github.com/imbus/robotframework-PlatynUI/commit/e1bd6b7224badaf19998b53d5cf21a1cee36a352))

  - platform.backend is read with a type check and warned about on a
    mismatch. One info record names the chosen backend, whether it was
    forced, the provider ids, the desktop bounds and the monitor count;
    the candidates and ignored components are debug.
  - The runtime warns when no platform backend can serve the session,
    naming the backends and what stops working, and when no provider is
    active. A construction option lets a test build of the extension
    report that situation itself.
  - A provider that fails to list its elements is reported once per
    episode and by name; its recovery is debug.
  - A pointer target clamped to the desktop warns with the requested and
    the actual coordinates. After a keyboard error, releasing the held
    keys is debug; an error is logged only when releasing fails.
  - Errors for a missing platform backend no longer name internal types.
  - Keyboard sequences keep the positions of their characters and keys. An
    error names the position, the character or key and the device's
    reason; a parse error gives the position and what is wrong in the
    user's terms and repeats no input, in Display or Debug.
    KeyboardActionError offers a rendering without any part of the
    sequence, for a Secret.

  Behavior change: keyboard errors give the position and no longer repeat
  the input.

- **runtime:** Per-runtime platform sessions, remove global lease ([a116981](https://github.com/imbus/robotframework-PlatynUI/commit/a116981d3afeabde7b98d1e89eb58cd64b485046))

  Each runtime now builds and owns its platform devices via a PlatformFactory
  (x11 / wayland / windows / mock) and releases them on drop, instead of
  borrowing process-global `&'static` devices gated by a reference-counted
  PlatformModulesLease. This deletes the lease and every process-global
  connection static -- the X11 connection OnceLock and the X11 + Windows
  highlight OnceLocks -- so a runtime built after a previous one is torn down
  always establishes a fresh connection.

  Fixes the multi-suite `just test-acceptance-x11` lane: was 16/26 failing
  with 'x11 connection: not available after shutdown or failed connect', now
  26/26 passing.

  - core: RuntimeConfig (platform/providers sections) + PlatformFactory /
    PlatformBundle; UiTreeProviderFactory::create(&RuntimeConfig) and the
    window manager are threaded per runtime (nodes reach the runtime's wm,
    no global).
  - x11: owned Arc<X11Connection>, per-instance highlight thread joined on
    drop. wayland/mock/windows: PlatformFactory wrappers. atspi: bus_address
    from providers.atspi.bus_address.
  - runtime: owns Option<PlatformBundle>; select_platform picks the backend by
    config.platform.backend or can_serve; shutdown drops the bundle.

- **runtime:** Add evaluate_single method for XPath evaluation ([2906a5d](https://github.com/imbus/robotframework-PlatynUI/commit/2906a5d077f8cdd46bda45978003c3eb5792e723))
- **runtime:** Implement shutdown lifecycle management and idempotency ([4bbf7b5](https://github.com/imbus/robotframework-PlatynUI/commit/4bbf7b5506c6003f32e7c26c8956cfbe4df63974))
- **runtime:** Finish mock fallback build gating ([b4e074f](https://github.com/imbus/robotframework-PlatynUI/commit/b4e074fc9771ccb5a912609e9f70a3f698bb5e65))
- **runtime:** Add keyboard sequence parser and API ([f63cfb8](https://github.com/imbus/robotframework-PlatynUI/commit/f63cfb827ed16e99c1dad812b53f9720f91bfb2b))
- **runtime:** Add window command and WindowSurface mocks ([c65e59f](https://github.com/imbus/robotframework-PlatynUI/commit/c65e59fb1ca9fea82568d5d865ac47d5554049bc))
- **runtime:** Add event capabilities to ProviderDescriptor and update runtime handling ([75cc15b](https://github.com/imbus/robotframework-PlatynUI/commit/75cc15b5b9aeb593808c6fd031764014be462391))
- **runtime:** Surface desktop info provider pipeline ([5bc2e1e](https://github.com/imbus/robotframework-PlatynUI/commit/5bc2e1eb484761ac9da02a2f68c13921220461a9))
- **runtime:** Add xpath evaluation bridge ([64d4abc](https://github.com/imbus/robotframework-PlatynUI/commit/64d4abcc043b38eca8099a2f0215f177e69c619c))
- **scripts:** Add Wayland session launcher using Weston ([787da1c](https://github.com/imbus/robotframework-PlatynUI/commit/787da1c41b9f50ea771e31eb95645fa38203da04))

  Add a startwaylandsession.sh script that launches an isolated Weston
  compositor session with full AT-SPI accessibility support, mirroring
  the existing Xephyr-based X11 session script.

  - Auto-detect host display server and choose the appropriate Weston
    backend (wayland, x11, or headless for CI)
  - Symlink parent Wayland socket into the isolated runtime directory
    so nested Weston can connect to the host compositor
  - Detect WSL and disable XWayland, which does not work reliably
    there
  - Start xdg-desktop-portal with GTK backend for working portal
    services (file chooser, screenshots, etc.)
  - Configure German keyboard layout and 1920x1080 resolution via a
    generated weston.ini

- **test-app-egui:** Expose a stable @Id on every interactive widget ([9ef3c71](https://github.com/imbus/robotframework-PlatynUI/commit/9ef3c7152a88c2788aaa491b8f5162a5e2ddae21))

  Add a WithAuthorId extension trait that attaches an AccessKit author_id to a
  widget's node via ctx.accesskit_node_builder(resp.id, ...); the id surfaces on the
  AT-SPI tree as the Control attribute @Id. Tag every interactive widget with a
  stable, semantic id (buttons, menu buttons + items, radios, inputs, checkboxes,
  toggle, sliders, drag value, combo box + items, progress bar, collapsing header,
  hyperlink, status labels). The dynamic label text stays in @Name; @Id is the
  stable, language-independent key acceptance tests locate by.

- **test-app-egui:** Expose window title on the AccessKit root node ([bc819dd](https://github.com/imbus/robotframework-PlatynUI/commit/bc819ddb0fdd3c2faa559cee1d5d562b58cd9f2e))

  egui creates the AccessKit window root without a label and does not propagate the OS window title to the accessibility tree, so the window had an empty name on AT-SPI and could not be located by title.

  Set the root node's label to the window title each frame via accesskit_node_builder(accesskit_root_id(), ...), so the window is discoverable by name (e.g. //*[@Name="PlatynUI Test App"]; AT-SPI role Frame).

- **test-apps:** Add Swing fixture app and JAB spike ([9766eba](https://github.com/imbus/robotframework-PlatynUI/commit/9766eba05298f8d837b5efd0ef11f18cf193700b))

  Add apps/test-app-swing (plain Java 8, javac via just recipes, excluded
  from the Cargo workspace like test-app-qt): stage-1/2 controls with
  fixed unique accessibleNames as the locator contract, a click
  observable, and CLI flags mirroring the other fixture apps. The run
  recipe enables the Java Access Bridge per process via
  -Djavax.accessibility.assistive_technologies, writing no persistent
  configuration.

  Add a throwaway JAB spike bin to crates/playground (force-added past
  the generic playground/ gitignore pattern) that validated DLL
  discovery, the dedicated pump-thread model, repr(C) struct layouts,
  and handle-release discipline, and harvested the fixture role
  vocabulary. Findings are recorded in the add-jab-provider change.

- **ui:** Add ItemContainer selection API, Deselectable pattern and chaining returns ([3541d06](https://github.com/imbus/robotframework-PlatynUI/commit/3541d06c08f5f920b174ecf8114e0c231c97b216))

  Rev. 44: introduce ItemContainer pattern with proxy-based item resolution;
  add column/row headers and table patterns; restructure ui/proxies.

  Rev. 45: add pattern consistency tests and adapter proxy registrations.

  Rev. 46: separate select/deselect semantics from add/remove_to_selection;
  ItemContainer exposes the four-method convenience API on Item subclasses.

  Rev. 47: split Deselectable into its own optional pattern; route deselect
  through Deselectable instead of MultiSelectable; action methods now return
  Self on Item and the resolved child I on ItemContainer to enable chaining
  (TreeItem -> 'TreeItem', Row -> 'Row | Cell', ComboBox.select -> ListItem).

- **ui:** Add TabList/TabItem and Menu/MenuBar/MenuItem contexts ([07ca742](https://github.com/imbus/robotframework-PlatynUI/commit/07ca742478a77fa7771af73cfb08c7024b752576))

  Adds five new UI context classes mirroring the legacy library:
  - TabList/TabItem: selectable tab navigation, aligned with lists.py API
  - Menu/MenuBar: container controls for menu hierarchies
  - MenuItem: interactive control with activate() that expands the
    ancestor chain outer-to-inner (skipping ancestors without the
    Expandable pattern) before invoking Activatable.activate() on self;
    walk stops at Window/DesktopBase boundaries.

  19 new tests (9 tabs + 10 menus). Total suite: 630 passed.

- **ui:** Add Item hierarchy, List, Tree, Table and ComboBox contexts ([c0f7552](https://github.com/imbus/robotframework-PlatynUI/commit/c0f7552040984a1ef860103801565196e0c5877a))

  Item marker with text property plus SelectableItem, ExpandableItem
  and EditableItem mixins. Container contexts List, Tree, Table
  (with Row/Cell/EditableCell) and ComboBox expose
  get_item(s)/iter_items/select against ItemContainer; ComboBox adds
  expand/collapse and in-place text editing.

  iter_items on ComboBox is a generator function so the dropdown
  stays expanded while the caller consumes items, fixing a latent
  bug in the legacy implementation that returned the iterator from
  within the expand/collapse context.

- **ui:** Add Text and Edit page-objects ([67643bb](https://github.com/imbus/robotframework-PlatynUI/commit/67643bb3738bb32bec3d6080ccb348a70ffa515d))

  Text wraps TextContent for read-only labels and status texts; Edit
  adds TextEditable + Clearable for editable fields with focus,
  not-readonly, in-view, and enabled predicates around set_text and
  clear. Both live in ui/text.py without an inheritance relationship.

  Moves is_multi_line from TextContent to TextEditable: the multi-line
  property is only behaviour-relevant for editable fields (tab vs.
  newline handling, line-break acceptance), while plain display texts
  treat it as a layout concern. Documents in core/patterns/text.py
  that the pattern hierarchy stays flat by design — relationships
  between capabilities are expressed via an adapter's
  supported_pattern_names, not via Python class inheritance, so the
  Reverse-DNS resolver behaves uniformly across all pattern pairs.

- **ui:** Add Button and CheckBox page-objects ([28898b2](https://github.com/imbus/robotframework-PlatynUI/commit/28898b2e0067b2fa008398074223b4352368eedc))

  AbstractButton provides the shared label/activate contract for
  button-like widgets; Button drives the Activatable pattern and
  CheckBox the Toggleable pattern with idempotent check/uncheck and
  a bounded set_state that terminates on two-state checkboxes.

  Renames a couple of test-local context classes that collided with
  the new auto-registered Button via __init_subclass__.

- **ui:** Add Element, Control, Window, Desktop and Application page-objects ([91e9257](https://github.com/imbus/robotframework-PlatynUI/commit/91e92577c6e0c9d5c5d07bac082784f8e0ded5fe))

  Introduces the `PlatynUI.ui` package with the foundational page-object
  hierarchy: `Element` and `Control` for the read-side capabilities,
  `Window`/`Frame` for top-level windows with the activate / minimize /
  maximize / restore / close / move / resize lifecycle, `DesktopBase` and
  `Desktop` for the screen root, and `Application` as the identity
  container that orchestrates graceful and forced shutdown.

  Adds the supporting pattern ABCs (HasUserInput, Closeable, Minimizable,
  Maximizable, Restorable, Movable, Resizable, Titled, Readable,
  ApplicationReady), re-exports `Size` from the native module, and
  introduces `Settings.application_exit_timeout` for the two-stage exit
  flow.

- **ui:** Add optional developer-provided stable identifier `Id` to UiNode and related attributes ([8ee7905](https://github.com/imbus/robotframework-PlatynUI/commit/8ee7905f1dcf12c9117f2f2dd8922d753dbf15a5))
- **uia:** Expose common attributes and gate pattern advertisement ([7b7f933](https://github.com/imbus/robotframework-PlatynUI/commit/7b7f933f1ff82824e628a5e6591f6f7795101f17))

  The architecture contract declares Technology and SupportedPatterns as
  always present on every control:/item: node. AT-SPI, JAB and the mock
  honor it; the UIA provider exposed neither, so `@Technology="UIAutomation"`
  silently matched nothing and the Swing dedup suite had to work around it.

  UIA nodes and the synthetic application node now carry both attributes,
  on the enumeration and the single-lookup path. SupportedPatterns reads
  back through the node's own supported_patterns() at value time, so the
  attribute and the advertisement cannot drift apart.

  Two adjacent defects go with it:

  Focusable was advertised unconditionally, so every static label claimed
  to be focusable. It is now driven by IsKeyboardFocusable — but read with
  ignoreDefaultValue, because that property's documented default is FALSE
  and the plain accessor cannot tell "the provider denies it" from "the
  provider never implemented it". Measured: static labels and title-bar
  buttons deny it explicitly, while an Electron window with accessibility
  off and several Win32 panes supply nothing at all. Treating silence as
  denial would have withdrawn a working capability from a whole window, so
  a silent top-level window keeps the pattern and a silent inner element
  does not.

  TextEditable is advertised as a capability marker for text-bearing
  elements that are not read-only, with control:IsReadOnly alongside it.
  No set-text action — text entry stays keyboard-driven. Note the opposite
  default from the focus gate, and deliberately so: there the question is
  whether to withdraw a capability the element may have, here whether to
  claim one, so an unanswerable read-only state counts as read-only.

  The text surface, its editability and the focus decision are cached per
  node like has_window_surface; without that, enumerating one node's
  attributes would repeat the same cross-process reads four times over.

  The core contract testkit gains a reusable common-attribute expectation,
  pinned against the mock provider. It is separate from verify_node
  because these attributes belong to no pattern: gating them behind a
  PatternExpectation would report a missing pattern instead of the missing
  attribute.

- **wayland:** Screenshot provider via the compositor control socket ([e2bb923](https://github.com/imbus/robotframework-PlatynUI/commit/e2bb923abec83a1378b056524124bd8a8c1ce75b))

  WaylandScreenshot now produces real screenshots when running under PlatynUI's
  own compositor: it asks the compositor for a full-frame base64 PNG over the
  control socket ({"command":"screenshot"}), decodes it (a self-contained base64
  decoder mirroring the compositor's encoder, plus the png crate) and crops it to
  the requested region scaled to physical pixels.

  Routing is compositor-aware — like highlight and window_manager it gates on the
  detected CompositorType (via SO_PEERCRED), not on the mere presence of the
  control socket (which a foreign or forwarded compositor could expose). Only
  CompositorType::PlatynUi uses the control socket; any other compositor returns
  CapabilityUnavailable. Phase 1b (docs/platform-linux-wayland.md §5) will slot
  ext-image-copy-capture-v1 / wlr-screencopy / xdg-desktop-portal in there.

  Unit tests cover the base64 decoder, the region crop, and the
  not-our-compositor routing. The status note in docs/platform-linux-wayland.md
  is updated accordingly.

- **wayland:** Add EIS-Server, type-text command, and xkb-util crate ([f0f7628](https://github.com/imbus/robotframework-PlatynUI/commit/f0f7628ccae665b50cf748af74de2c3849c07da0))

  Implement EIS-Server in the compositor for libei-based input injection,
  add type-text command to the EIS test client, and introduce the
  platynui-xkb-util crate for XKB reverse lookup with compose support.

  EIS-Server (~370 LoC):
  - Full input capabilities: pointer, pointer_absolute, button, scroll,
    keyboard, touchscreen
  - XKB keymap propagation to clients
  - Output regions for absolute pointer mapping
  - Single-client with replacement on new connection

  type-text command (eis-test-client):
  - Unicode text input via XKB reverse lookup (KeymapLookup)
  - Compose sequence support (dead_grave + a → à, dead_acute + e → é)
  - Keymap source cascade: device keymap → CLI flags → env vars → us
  - 13 CLI subcommands, 14 interactive REPL commands

- **wayland-compositor:** Identify the compositor in the status response ([e095b63](https://github.com/imbus/robotframework-PlatynUI/commit/e095b6390bcc95aac4e9cfa0edc1aa1a7e3d67dc))

  The status response, which `ping` aliases, now carries
  "compositor": "platynui". A client that reaches a control socket can
  tell a PlatynUI compositor from any other JSON-lines peer that answers
  {"status":"ok"} with one round trip, without seeing the compositor's
  process, which it cannot when the two run in different PID namespaces.
  The marker does not depend on process name, path or version, and no
  other command returns it. Every existing field is unchanged, so current
  clients and platynui-wayland-compositor-ctl keep working.

  The IPC protocol documentation describes the field and what clients use
  it for.

- **wayland-compositor:** Add touch input and SSD touch interaction ([fa33cad](https://github.com/imbus/robotframework-PlatynUI/commit/fa33cad18d84c8943397ae4378ffdb58f64dcba0))

  Add full touchscreen support (~385 LoC across 4 files):

  - seat.add_touch() capability with touch() helper method
  - Backend and EIS touch handlers using shared process_touch_*()
    functions for unified behavior across input sources
  - surface_under_point() refactored as pub(crate) hit-test utility
  - TouchMoveSurfaceGrab with incremental delta re-anchoring and
    dual-output dead-zone protection for L-shaped multi-monitor setups
  - TouchResizeSurfaceGrab supporting all 12 resize directions
  - Deferred SSD button actions (Close/Maximize/Minimize) with
    slot verification and continuous position tracking
  - Touch coordinates via combined_output_geometry(), independent
    of pointer position

- **wayland-compositor:** Add ext-data-control-v1 clipboard protocol ([bf89a62](https://github.com/imbus/robotframework-PlatynUI/commit/bf89a62456e1d21fef88e8d73292db3aa9244a63))

  Implement ext-data-control-v1 alongside the existing wlr-data-control-v1.
  Both protocols provide identical clipboard management functionality:
  - wlr-data-control-v1: wlroots-originated, widely adopted (Sway, wlr-based)
  - ext-data-control-v1: standardized staging version (Mutter, KWin)

  Uses smithay's delegate_ext_data_control!() macro with a type alias
  (ExtDataControlState) to avoid name collision with the wlr version.
  Both protocols are offered in parallel for maximum client compatibility.

- **wayland-compositor:** Add xdg-toplevel-icon-v1 and xdg-toplevel-tag-v1 protocols ([f229a34](https://github.com/imbus/robotframework-PlatynUI/commit/f229a349f973fddfae3108e8e215735970c9aa71))

  Implement two Tier 3 staging protocols (42 globals total):

- **wayland-compositor:** Upgrade wl_compositor to version 6 ([13b97ae](https://github.com/imbus/robotframework-PlatynUI/commit/13b97aeacf15c63d3efee59f21d46064e564e191))

  Upgrade from CompositorState::new() (v5) to CompositorState::new_v6()
  which adds preferred_buffer_scale and preferred_buffer_transform
  support.

  Call send_surface_state() in the commit handler to notify clients of
  the preferred output scale and transform, matching the behavior of
  production compositors like Mutter.

- **wayland-compositor:** Implement tearing-control and toplevel-drag stubs ([b58f603](https://github.com/imbus/robotframework-PlatynUI/commit/b58f6034d07ce73e869dd2be291699584f90ee0f))

  Add manual GlobalDispatch/Dispatch implementations for two staging
  protocols that smithay 0.7 does not yet abstract:

  - wp-tearing-control-v1: accepts presentation hints (vsync/async) but
    silently ignores them — prevents protocol-not-found warnings from
    Chromium, games, and Vulkan apps.

  - xdg-toplevel-drag-v1: accepts attach requests (toplevel + offset)
    and logs them, but window-during-drag behaviour is not yet wired —
    prevents warnings from Firefox/Chromium tab-detach operations.

  Both use the same pattern as pointer-warp-v1: wayland-protocols 0.32
  bindings reexported by smithay, GlobalDispatch for the manager, Dispatch
  for the per-resource object.

  Compositor now advertises 40 protocol globals.

- **wayland-compositor:** Implement pointer-warp-v1 protocol ([34c02d9](https://github.com/imbus/robotframework-PlatynUI/commit/34c02d9fa89c999d16a12c7f752caf1e8e370d68))

  Add manual GlobalDispatch/Dispatch implementation for the
  wp_pointer_warp_v1 staging protocol using wayland-protocols 0.32
  bindings reexported by smithay (smithay 0.7 does not yet provide a
  high-level abstraction for this protocol).

  The handler translates surface-local coordinates to global compositor
  coordinates, updates the pointer location, and sends a motion event
  via PointerHandle. Security policy filtering is applied via can_view.

  Used by accessibility tools, remote-desktop clients (mutter already
  implements this protocol), and application drag operations.

- **wayland-compositor:** Add Tier 2 protocols (pointer-gestures, tablet, xwayland-keyboard-grab) ([b84d0c9](https://github.com/imbus/robotframework-PlatynUI/commit/b84d0c9a4b8c0d16ce8d2247f8caad60efab81c9))

  Implement the remaining Tier 2 protocol extensions, bringing the total
  to 37 delegate macros:

  - pointer-gestures-v1: delegate-only, smithay routes swipe/pinch/hold
    events via PointerHandle automatically
  - tablet-v2: TabletManagerState + delegate macro (TabletSeatHandler
    already existed for cursor-shape)
  - xwayland-keyboard-grab: XWaylandKeyboardGrabHandler with
    keyboard_focus_for_xsurface() that maps WlSurface → X11 Window,
    lazy-initialized alongside XWayland

  Update plan: Tier 2 marked complete, gap analysis updated to 37/44.

- **wayland-compositor:** Implement Phase 3b Tier 1 protocols ([6466c5f](https://github.com/imbus/robotframework-PlatynUI/commit/6466c5fda21ceaf94b89c1bacd8dcd56d777ad2a))

  Add six additional Wayland protocols required by GTK4/Qt/Chromium apps:

  - wp-commit-timing-v1: frame timing (smithay-internal, delegate-only)
  - wp-fifo-v1: FIFO scheduling (smithay-internal, delegate-only)
  - wp-alpha-modifier-v1: subsurface opacity (smithay-internal, delegate-only)
  - zwp-idle-inhibit-v1: idle inhibition with proper tracking via
    HashSet<WlSurface> and set_is_inhibited() on IdleNotifierState;
    also wire notify_activity() into input processing
  - xdg-dialog-v1: modal dialog enforcement — find_modal_child() walks
    the modal chain recursively, focus_and_raise() redirects focus from
    parent to modal child, SSD header/resize interactions on the parent
    are blocked while a modal is open
  - xdg-system-bell-v1: bell notification (log-only, no audio backend)

- **wayland-compositor:** Automation protocols for remote-accessible UI testing ([27fe406](https://github.com/imbus/robotframework-PlatynUI/commit/27fe406544713bc76500548ddffd42a57c33c826))

  Enable test harnesses to discover windows, inject input, capture
  screenshots, and manage outputs through standard Wayland protocols.
  All protocols required for remote access via wayvnc are now
  implemented, allowing VNC connections to headless CI sessions.
  Error handling is production-grade — no silent failures or panics
  in unattended runs.

- **wayland-compositor,inspector:** Improve winit window and desktop integration ([9cf6ba1](https://github.com/imbus/robotframework-PlatynUI/commit/9cf6ba191e065fa083c9eed6cc4af55f88d4d13e))

  Customize the winit backend window for proper desktop environment
  integration on GNOME and KDE Plasma:

  - Set window title to "PlatynUI Wayland Compositor" (was "Smithay")
  - Enable Adwaita-themed CSD via wayland-csd-adwaita feature
  - Detect system dark/light mode via XDG Desktop Portal (zbus)
  - Embed application icon (PNG decoded at startup)
  - Standardize app_ids to org.platynui.* across all binaries:
    compositor (org.platynui.compositor), inspector
    (org.platynui.inspector), test-app (org.platynui.test.egui)

  Add freedesktop .desktop files for compositor and inspector so that
  Wayland compositors (KWin, GNOME Shell) can resolve window icons
  via app_id lookup.

  New dependencies (compositor):
  - winit 0.30 (direct, for wayland-csd-adwaita feature unification)
  - zbus 5 (D-Bus queries for color-scheme detection)

- **window:** Add bring-to-front functionality with optional wait time for window activation ([f6422ef](https://github.com/imbus/robotframework-PlatynUI/commit/f6422efc5e331bc6bf99152ef1f9ed0209e011c9))
- **window-state:** Add is_modal and split Rust attributes ([3580e1e](https://github.com/imbus/robotframework-PlatynUI/commit/3580e1e37ec8e961b4ece56f6f78015912b09959))

  Add is_modal to WindowState (UIA CurrentIsModal, AT-SPI STATE_MODAL)
  and split the Rust attribute module attributes::activatable into two:
  attributes::activatable retains IS_ACTIVATION_ENABLED and
  DEFAULT_ACCELERATOR; the new attributes::window_state hosts IS_ACTIVE,
  IS_TOPMOST, and IS_MODAL.

  Fix the AT-SPI provider where the IsTopmost attribute previously
  returned the foreground/active flag; rename it to IsActive and add a
  real IsModal via State::Modal. Implement IsActive (foreground-window
  comparison via GetForegroundWindow) and IsModal (CurrentIsModal) in
  the Windows UIA provider. The macOS AX provider remains a stub.

  Designdoc Rev. 39 documents the addition.

- **windows:** Implement Desktop Provider ([6270687](https://github.com/imbus/robotframework-PlatynUI/commit/6270687f2b97fc1331c310a377dd7e8f8d6edf69))
- **windows:** Implement screenshot provider ([0d4e43f](https://github.com/imbus/robotframework-PlatynUI/commit/0d4e43fceb21242b9411a533970bb0105e42577e))
- **windows:** Implement highlight provider ([ad85912](https://github.com/imbus/robotframework-PlatynUI/commit/ad85912d43d1ae2eedb2b860ee592e46063568b8))
- **windows:** Centralize dpi setup and register pointer device ([388e641](https://github.com/imbus/robotframework-PlatynUI/commit/388e64112079dd900bd09549e10f57a2cc243279))
- **windows-uia:** Implemented first version of UIAutomation Provider ([2d575d7](https://github.com/imbus/robotframework-PlatynUI/commit/2d575d79d6d5db4d258219d4fadb88d66a55c4e3))
- **xpath:** Propagate evaluation errors in XPath evaluation and iterators ([8b9aab5](https://github.com/imbus/robotframework-PlatynUI/commit/8b9aab5957d68d6d20059f4863e7adfa24bcbb41))
- **xpath:** Implement direct attribute lookup by expanded QName ([0fb4714](https://github.com/imbus/robotframework-PlatynUI/commit/0fb47142b8a442c77b12e0e22902ba1477969bcd))
- **xpath:** Introduce EvaluationStream for owned XPath evaluation results ([bca283f](https://github.com/imbus/robotframework-PlatynUI/commit/bca283fd075f5b35d02ccfed12d91fb1dfb413f3))
- **xpath:** Implement real streaming of results ([dcc8eca](https://github.com/imbus/robotframework-PlatynUI/commit/dcc8eca4d311983d718f9b26e6b4566aa4ea78ec))
- **xpath:** Stream typed atomics for runtime evaluation ([da8b1cf](https://github.com/imbus/robotframework-PlatynUI/commit/da8b1cfefb2aad1e5af51dfe43a83886cfc0166c))
- **xpath:** Enforce static context item type checks ([cd79395](https://github.com/imbus/robotframework-PlatynUI/commit/cd79395f2776e7b973cb3c6eaae35a9b3787151b))
- **xpath:** Enforce XPath function conversion rules ([2e67ee6](https://github.com/imbus/robotframework-PlatynUI/commit/2e67ee6aaf3914565ef34b2375a4dfe2cf18d5a1))
- **xpath:** Implement parameter kind tracking and atomization for function calls (first version) ([6412209](https://github.com/imbus/robotframework-PlatynUI/commit/64122090925a44e954e54780c138c8d00b6c440e))
- **xpath:** Implement compile cache in StaticContext for performance optimization ([9c62994](https://github.com/imbus/robotframework-PlatynUI/commit/9c6299402db034dd25a068abc1c2aa56a6a433ae))
- **xpath:** Finalize extended casting support and some speed optimizations ([87a1f02](https://github.com/imbus/robotframework-PlatynUI/commit/87a1f027184a15d3dbeb433913630b5dd7c7df48))
- **xpath:** Add SingleItemCursor and RangeCursor for XdmSequenceStream, enhance chaining functionality ([330bd2c](https://github.com/imbus/robotframework-PlatynUI/commit/330bd2c979c9a669021345cfeff70afa939f2f46))
- **xpath:** Implement function signatures catalogue and statically known collations ([7d7fcc2](https://github.com/imbus/robotframework-PlatynUI/commit/7d7fcc2249adeac19fe791cf45f64b2edb44a5bb))
- **xpath:** Add support for filter expressions in path steps and implement corresponding evaluation logic ([1314585](https://github.com/imbus/robotframework-PlatynUI/commit/13145853a83dc0661449a6bf19d29af8fcb0d7a2))
- **xpath:** Implement 'let' expression ([ea60d59](https://github.com/imbus/robotframework-PlatynUI/commit/ea60d5979ef11117c9a4e33909abde2e5945dd31))
- **xpath:** Implement variable scoping and error handling for undeclared variables ([59cc134](https://github.com/imbus/robotframework-PlatynUI/commit/59cc13429333e54865693c12657409fc442393e0))
- **xpath:** Align fn implementations with W3C F&O 2.0 ([9b551ba](https://github.com/imbus/robotframework-PlatynUI/commit/9b551baccc801d9d2a23236cd85203f997910d6a))
- **xpath:** Add some remaining xpath functions and optimizations ([88b584b](https://github.com/imbus/robotframework-PlatynUI/commit/88b584b630cc87fd5416db277bec386b42c64412))
- **xpath:** First most complete XPath evaluator ([50afd00](https://github.com/imbus/robotframework-PlatynUI/commit/50afd0020b843aa368bf56a5353be90b71766f72))
- **xpath:** Rewrite of parts of the evaluator ([b900e4a](https://github.com/imbus/robotframework-PlatynUI/commit/b900e4a8937aaaaa9ee244077bf4096fd2bfa32f))
- **xpath:** Complete rewrite of compiler ([ecbfca7](https://github.com/imbus/robotframework-PlatynUI/commit/ecbfca7283381826d218cfaf07f3c3c7222768f8))
- **xpath:** Complete rewrite of xpath parser ([a1dc405](https://github.com/imbus/robotframework-PlatynUI/commit/a1dc405f8fc2577d883205564fdbe60590b28fd9))
- **xpath:** Implement XPath 2.0 Temporal Functions and Types ([04dba8f](https://github.com/imbus/robotframework-PlatynUI/commit/04dba8f92549ca4760e9f39f9ace545e64ec8f83))

  - Added support for dateTime, date, time, yearMonthDuration, and dayTimeDuration types in the XDM model.
  - Introduced functions for extracting components from dateTime and date types (year, month, day) and time types (hours, minutes, seconds).
  - Implemented implicit-timezone function to retrieve the effective timezone.
  - Enhanced string representation for date, time, and dateTime types, including timezone formatting.
  - Added comprehensive tests for datetime arithmetic, comparisons, casting, and edge cases.
  - Included error handling for invalid date and time formats.

- **xpath:** Implement collation functions and regex support in XPath evaluator ([16bb621](https://github.com/imbus/robotframework-PlatynUI/commit/16bb621da8f18e2072501810645a49589afcb216))

  - Added new collation types: SimpleCaseCollation, SimpleAccentCollation, and SimpleCaseAccentCollation with appropriate comparison and key methods.
  - Registered built-in simple collations in the CollationRegistry.
  - Enhanced the RegexProvider with methods for matching, replacing, and tokenizing strings using Rust's regex library.
  - Introduced DynamicContext fields for current date/time handling and timezone overrides.
  - Added tests for collation-aware functions including contains, starts-with, ends-with, compare, and codepoint-equal.
  - Implemented tests for current-dateTime, current-date, and current-time functions with fixed 'now' and timezone overrides.
  - Created tests for regex functions including matches, replace, and tokenize, covering various flags and error cases.

- **xpath:** Make try_compare_by_ancestry public ([a128af0](https://github.com/imbus/robotframework-PlatynUI/commit/a128af02a1751ba694c6b9e53786bbf284c4cd25))
- **xpath:** Refactor XPath Function Implementation and Enhance Document Order Comparison ([2699ada](https://github.com/imbus/robotframework-PlatynUI/commit/2699ada623b2ef382b96200f8832b5b87a8599df))

  - Introduced CallCtx struct to provide context for function implementations, including dynamic and static contexts, default collation, resource resolver, and regex provider.
  - Updated FunctionImpl type to accept CallCtx as a parameter.
  - Modified compare_document_order method in XdmNode trait to return Result<Ordering, Error> instead of Ordering, allowing for error handling in node comparisons.
  - Adjusted multiple test implementations to align with the new compare_document_order signature.
  - Added new tests for multi-root error handling and function context exposure, ensuring proper functionality of the new CallCtx structure.
  - Updated documentation to reflect changes in the API and implementation status, including the transition to context-aware function signatures.

- Enhance OS version retrieval for Unix systems ([c0d4e0b](https://github.com/imbus/robotframework-PlatynUI/commit/c0d4e0b4205f6b72ead73c6b10d2a4514719b92f))
- First version of the platynui-wayland-compositor ([1c4f55b](https://github.com/imbus/robotframework-PlatynUI/commit/1c4f55bab41adad6b93eaf5738c0920d04e91a52))
- Implement graceful resource cleanup for providers and platforms ([2602643](https://github.com/imbus/robotframework-PlatynUI/commit/26026436d5f7346768bd8523baf75be0f1cecb12))


  Add shutdown() to providers and platform modules so OS resources are
  released deterministically rather than relying on process exit.

  - provider-atspi: ClearableCell-based connection with shutdown guard
  - provider-windows-uia: AtomicBool guard, clear COM thread-locals
  - core: add PlatformModule::shutdown() default method
  - runtime: call platform module shutdown after provider shutdown
  - platform-linux-x11: teardown X11 connection and highlight thread
  - platform-windows: teardown highlight thread and HWND
- Integrate tracing for enhanced logging across modules ([4f065bc](https://github.com/imbus/robotframework-PlatynUI/commit/4f065bce69af472c04c05f3b16036b04ffc4f64f))
- Cache queries and requery support ([29eedd3](https://github.com/imbus/robotframework-PlatynUI/commit/29eedd3659a5723c4102fec43129bc34a6158879))
- Stabilize dbus connection and add some tracing informations ([87eb514](https://github.com/imbus/robotframework-PlatynUI/commit/87eb514d833cfa73541cd484f9ef195476a209ac))
- Implement core AT-SPI support ([58004c9](https://github.com/imbus/robotframework-PlatynUI/commit/58004c9d2dc6fd34c22bc9d303ab18904edcd403))
- Initial Linux/X11 platform support with keyboard, pointer, screenshot, and highlight devices ([e61c500](https://github.com/imbus/robotframework-PlatynUI/commit/e61c500ad3631026ec7a7afe2288b0b2cd24d730))
- Add project URLs to pyproject.toml files ([61cf164](https://github.com/imbus/robotframework-PlatynUI/commit/61cf16449c8c9a933a2a60ab5db91e554d0eab24))
- Introduce platynui-cli and platynui-inspector tools as separate installable python packages ([c58da55](https://github.com/imbus/robotframework-PlatynUI/commit/c58da55d9769e9694d59fcaf6c261b61a473f8a5))
- First version of spy tool part 3 ([345c3e2](https://github.com/imbus/robotframework-PlatynUI/commit/345c3e28f471eb9b213478b363b789ab53516b99))
- First version of spy tool part 2 ([f4b38e1](https://github.com/imbus/robotframework-PlatynUI/commit/f4b38e1cdc1f5287d181c29d5de304deb5540186))
- First simple version of spy tool ([ec5d124](https://github.com/imbus/robotframework-PlatynUI/commit/ec5d124bb3807195ff31183c36063cefd40a6057))
- Add update version and changelog scripts ([6065757](https://github.com/imbus/robotframework-PlatynUI/commit/606575777482385c0a555c1c866e5fd93bbaa173))
- Add CI workflow for building, testing, and packaging Python and Rust projects ([e5c1f39](https://github.com/imbus/robotframework-PlatynUI/commit/e5c1f396acc5e013f4d6c354462b8e81b435de8f))
- Add duration-aware highlighting pipeline ([57e4aac](https://github.com/imbus/robotframework-PlatynUI/commit/57e4aac6b8d91e17b2212e40f0ef92f023de4e56))
- More steps for out own XPath2Parser ([103e779](https://github.com/imbus/robotframework-PlatynUI/commit/103e779e52f958f275596199b96352d596cd8c3b))
- First steps for our own xpath evaluator ([411105b](https://github.com/imbus/robotframework-PlatynUI/commit/411105b1530b6fefd7a18fe3ecba77a157a4e94e))
- Enhance string literal parsing to normalize doubled quotes in XPath expressions ([f105df9](https://github.com/imbus/robotframework-PlatynUI/commit/f105df9768f527b69500063ca42952aee5f03bc0))
- Enhance XPath2 grammar with structured keywords and operator rules ([fd320b5](https://github.com/imbus/robotframework-PlatynUI/commit/fd320b56976f896e776368f596fe3264adfa9cfe))
- Initialize project ([c83eab4](https://github.com/imbus/robotframework-PlatynUI/commit/c83eab4342aaa40871bfa5fc73c977df1347fbfa))


### Performance

- **cli:** Optimized cli query output ([2fb1173](https://github.com/imbus/robotframework-PlatynUI/commit/2fb11734c39587e027a1c93fc4fdaa51dbbb8726))
- **doc-order:** Use doc_order_key for distinct and sets ([9955fcb](https://github.com/imbus/robotframework-PlatynUI/commit/9955fcb6cfe905e8a96846dc9ada411d5f61d4e7))
- **evaluator:** Stream axis traversal buffer ([3da02e5](https://github.com/imbus/robotframework-PlatynUI/commit/3da02e565b5d4219762daf33caeaa65547124479))
- **evaluator:** Reuse vm frame overlays ([a322d70](https://github.com/imbus/robotframework-PlatynUI/commit/a322d70778b1df2dc64f9c8480268f948a4520f7))
- **evaluator:** Switch VM stacks to SmallVec and update plan ([4418475](https://github.com/imbus/robotframework-PlatynUI/commit/4418475c1199f4b967f05d38811e44b294d30879))
- **parser:** Reduce Pair cloning in AST builder ([f23f814](https://github.com/imbus/robotframework-PlatynUI/commit/f23f814169be629d2e270e0b8bb03497904daf7a))
- **provider-atspi:** Lazy child resolution and functional cache invalidation ([f0dea78](https://github.com/imbus/robotframework-PlatynUI/commit/f0dea78bf6dd29da6b93de625003b499048d0a0a))

  Introduce ClearableCell<T>, a Mutex-based cache cell that supports
  clearing unlike OnceCell. Replace OnceCell with ClearableCell for all
  volatile node properties (state, interfaces, name, child_count,
  process_id).

  Remove eager resolve_basics() calls from children() iterator. Child
  nodes are now returned as bare shells with all properties resolved
  lazily on first access. This avoids 3–4 unnecessary D-Bus roundtrips
  per child node when the XPath engine does not need those properties.

  Make invalidate() functional: it now clears all ClearableCell fields
  so that subsequent accesses re-query the accessibility bus. Previously
  invalidate() was a no-op because OnceCell does not support clearing.

  Raise slow-operation warning threshold from 200 ms to 1000 ms to
  reduce noise in typical AT-SPI2 environments.

- **runtime:** Implement RuntimeXdmNode caching ([2a33bd1](https://github.com/imbus/robotframework-PlatynUI/commit/2a33bd123bc6595fa0a8f29b72daae28a9f7d83f))
- **set-ops:** Hash-based union/intersect/except ([4c8d0b6](https://github.com/imbus/robotframework-PlatynUI/commit/4c8d0b6ac25200b109aa8b92f87f63ab58cd3b61))
- **set-ops:** Use doc_order_key and smallvec buffers ([87d42a7](https://github.com/imbus/robotframework-PlatynUI/commit/87d42a75a0446f7b38ec93b5cf52031739789ecd))
- **set-ops:** Smallvec buffering for axes and doc-order keys ([e5cce15](https://github.com/imbus/robotframework-PlatynUI/commit/e5cce15c4a0cf212179d8c8179babf93a5fdf72d))
- **xpath:** Zero-copy string handling via Cow across XPath engine ([0e7f0b3](https://github.com/imbus/robotframework-PlatynUI/commit/0e7f0b3e76c14fb6c3765d0e93d15037ad6ba999))
- **xpath:** Avoid unnecessary cloning in compare_atomic via Cow ([52cf29b](https://github.com/imbus/robotframework-PlatynUI/commit/52cf29b130e0b515bb3516a06160090f9f41b192))

  Replace eager clone of both operands in the normalization catch-all
  with Cow::Borrowed. Cow::Owned is only produced when UntypedAtomic
  values actually need normalization. This eliminates allocations for
  QName, NOTATION, DateTime, Duration, Date, and Time comparisons.

- **xpath:** Avoid double-clone of compiled instructions in compile cache ([5614880](https://github.com/imbus/robotframework-PlatynUI/commit/561488034d6feeda3f5dfff399c8e038fdae7fb7))

  When populating the compile cache on a cache miss, the instruction
  sequence was cloned to create the Arc cache entry while the original
  was returned separately. Wrap the instructions in an Arc first,
  clone the Arc (cheap refcount bump) for the cache, and use
  Arc::unwrap_or_clone for the return value. This eliminates one full
  deep clone of the instruction sequence per cache miss.

- **xpath:** Eliminate unnecessary allocations in sequence helpers ([241205c](https://github.com/imbus/robotframework-PlatynUI/commit/241205c78b33a5e4d65ecc43f5b58ef1d8221d45))

  - reverse(): use in-place Vec::reverse instead of collecting a new reversed iterator
  - id()/idref(): extend DFS stack directly instead of allocating intermediate Vec for children
  - deep-equal(): compare children streaming instead of materializing both child lists

- **xpath:** Remove legacy function registration and streamline to use stream-based functions ([0397883](https://github.com/imbus/robotframework-PlatynUI/commit/0397883df8812a26ba9ea00d74fa2c8bc288bf19))
- **xpath:** Convert the rest of the xpath functions to streaming versions ([44ebe2a](https://github.com/imbus/robotframework-PlatynUI/commit/44ebe2aaa2a40f2676caff04edb4e731ce8f4281))
- **xpath:** Implement more streamed xpath functions ([b0ed405](https://github.com/imbus/robotframework-PlatynUI/commit/b0ed405083fcbc3d56def0f89a225176fcab68b9))
- **xpath:** Implement Predicate-Pushdown-Optimizer and Short-circuit Evaluation ([bb641bf](https://github.com/imbus/robotframework-PlatynUI/commit/bb641bf9bd9ace4610d0d69dcc22e826f8669c08))
- **xpath:** Stream axis/path; remove unwraps ([335ecef](https://github.com/imbus/robotframework-PlatynUI/commit/335ecef035ba10887e25b122fdbf4295aa384dcf))
- **xpath:** Stream evaluator end-to-end; add optimistic doc-order hint ([564493e](https://github.com/imbus/robotframework-PlatynUI/commit/564493eabe59351d4383e44ac61fb99648c9d2b3))
- **xpath:** Streamline doc order and predicate lowering ([754de09](https://github.com/imbus/robotframework-PlatynUI/commit/754de090d1855700c279e75de1e64954cf342fa3))
- **xpath:** Shortcut doc-order distinct when already sorted ([cbd2e36](https://github.com/imbus/robotframework-PlatynUI/commit/cbd2e360b735a2f04587c5ef962aa973ef014c50))
- **xpath:** Reuse doc-order keys in distinct ([746b253](https://github.com/imbus/robotframework-PlatynUI/commit/746b2538a30998f36584f5178422094b0ee841d0))
- **xpath:** Cache fancy regex compilations ([8d4771a](https://github.com/imbus/robotframework-PlatynUI/commit/8d4771adf2e44ef84fa1b2a2debe412b97ad9e0b))
- **xpath:** Optimize set operators with doc-order merges ([bb2fd7e](https://github.com/imbus/robotframework-PlatynUI/commit/bb2fd7ef857076eb81e108eb107a4ca6f3502f8c))
- **xpath:** Precompute document order metadata ([eb7065c](https://github.com/imbus/robotframework-PlatynUI/commit/eb7065cb62662f1011b8da1619ec408472ad46e4))
- Improve compiler scope stack and set-union merge ([8dc3a48](https://github.com/imbus/robotframework-PlatynUI/commit/8dc3a48a9f400f7fcdb54fd83a05cdb74ea54e51))


### Refactor

- **BareMetal:** Remove ListenerV3 dependency and improve error handling in highlight method ([323a063](https://github.com/imbus/robotframework-PlatynUI/commit/323a063d87dc816b4b1c3f2d5ed911e7030f06c4))
- **cli:** Adopt the workspace lints ([0cf7b92](https://github.com/imbus/robotframework-PlatynUI/commit/0cf7b926f77eb2daa0a7974f3f7ae5ceb9c0d038))

  platynui-cli never opted into `[workspace.lints]`, so `unsafe_code =
  deny`, `unused_crate_dependencies` and the pedantic set did not apply to
  it. It inherits them now; findings are fixed in code, and where the code
  is right as it is, the lint is allowed on the smallest item with the
  reason beside it.

  tracing is dropped as unused. Test and mock-provider builds leave the OS
  provider crates unlinked on purpose, so the unused-dependency lint is
  allowed only for those builds.

- **cli:** Simplify attribute handling in pointer commands and improve iterator type definitions in XPath ([58f7814](https://github.com/imbus/robotframework-PlatynUI/commit/58f781416754fbfe2c294d9389ab57d06f844660))
- **cli:** Remove unused filters ([1abd0e8](https://github.com/imbus/robotframework-PlatynUI/commit/1abd0e8fb6dd6bca4f73c87058dbccafb1f22e9c))
- **cli:** Split subcommands into modules ([0927dc9](https://github.com/imbus/robotframework-PlatynUI/commit/0927dc942585fcc3883c209174ebc65fee9c27e4))
- **cli,platform-mock:** Unify CLI output; move logs to mocks ([9fb5817](https://github.com/imbus/robotframework-PlatynUI/commit/9fb5817971200b0ad6e795d898632c11342b8901))
- **compositor:** Switch DRM backend to VBlank-driven event_loop.run() ([273a626](https://github.com/imbus/robotframework-PlatynUI/commit/273a62640187c52222e9bad05649706331106ab8))

  Replace the 16ms polling while-loop with calloop's event_loop.run()
  and a render_ping for VBlank-driven frame scheduling. Rendering now
  only happens when there is actual work: client commits trigger
  schedule_render(), VBlank completions schedule the next frame, and
  output config changes request a redraw.

- **compositor:** Switch headless backend to event_loop.run() ([86ca7a5](https://github.com/imbus/robotframework-PlatynUI/commit/86ca7a5f9db5e7144cb2d899a1021728aa1445ca))

  Replace the while-loop with calloop's event_loop.run() for consistency
  with the winit backend. Frame callbacks are sent in the idle callback
  after every event-loop iteration for immediate responsiveness. A periodic
  16ms timer keeps the loop alive when no client events arrive.

- **compositor:** Rewrite winit backend event loop for on-demand rendering ([bb0a4cb](https://github.com/imbus/robotframework-PlatynUI/commit/bb0a4cb36db386e663ad753266337b8445bede71))

  Rewrite the winit backend from a synchronous while-loop to an
  event-driven architecture using two calloop ping sources:

  - event ping: pumps winit events, re-pings itself on Continue
  - render ping: renders one frame, triggered only by visual changes

  The old loop rendered every iteration and relied on a 1ms dispatch
  timeout, coupling rendering tightly to protocol processing. The new
  architecture decouples them: the calloop sleeps on the Wayland display
  fd between pings, processes client messages at full speed, and only
  renders when something visual actually changed.

- **compositor:** Split context menu into GPU solids + CPU text overlay ([6bbdf1a](https://github.com/imbus/robotframework-PlatynUI/commit/6bbdf1a9f0fdbd752bb5d3ce698d6a78f7983b23))

  Move context menu background, border, separator, and hover highlight
  to SolidColorRenderElement (GPU-native). Only text remains in the
  transparent MemoryRenderBuffer overlay. Remove unused draw_rounded_rect.

- **compositor:** Replace egui with tiny-skia/swash for titlebar rendering ([3a71eb5](https://github.com/imbus/robotframework-PlatynUI/commit/3a71eb5758d560985b74093482965dddd9854960))

  Remove egui and egui_glow dependencies from the wayland compositor.
  Titlebars are now rendered using tiny-skia for 2D drawing and swash
  for font shaping/glyph rasterization.

  - Split titlebar into GPU-native SolidColorRenderElement (background,
    button fills) and a transparent MemoryRenderBuffer overlay (text,
    button symbols, icon)
  - Hover/focus changes only update solid element colors — no CPU
    repaint needed unless title or icon changes
  - Output premultiplied RGBA directly to avoid washed-out antialiased
    edges on the transparent overlay
  - Upgrade tiny-skia from 0.11 to 0.12
  - Remove GlowRenderer dependency from ui.rs
  - Document wayland-csd-adwaita requirement (GNOME has no SSD)

- **core:** Adopt the workspace lints ([1562f7e](https://github.com/imbus/robotframework-PlatynUI/commit/1562f7efef64c764ce61c917fbc25822c782af5a))

  platynui-core never opted into `[workspace.lints]`, so `unsafe_code =
  deny`, `unused_crate_dependencies` and the pedantic set did not apply to
  it. It inherits them now; findings are fixed in code, and where the code
  is right as it is, the lint is allowed on the smallest item with the
  reason beside it.

  Fallible public APIs now document their errors and panics.

- **core:** Drop the programmatic set-text pattern ([2cf681f](https://github.com/imbus/robotframework-PlatynUI/commit/2cf681f96d8307b24a442bb16e072f1d660cea68))
  - **BREAKING**: drop the programmatic set-text pattern

  PlatynUI enters text the way a user does: focus, select-all, type. The
  JAB provider nevertheless implemented TextEditable as a programmatic
  write, and the core trait documented that as the intent ("as opposed to
  synthesizing keystrokes"). That was an oversight — no Python/Robot code
  path could reach the action, it bypasses the application's real input
  handling, and it invited every future provider to repeat it.

  TextEditable becomes a pure capability marker: still advertised in
  SupportedPatterns for elements that genuinely accept input, paired with
  IsReadOnly, but without a pattern instance — which the node contract
  explicitly allows (runtime_pattern_without_instance_is_allowed).

  - core: remove TextEditablePattern/TextEditableAction and their tests;
    the marker semantics are documented on pattern_names::TEXT_EDITABLE,
    the one symbol every provider touches to advertise the pattern.
  - provider-jab: remove the pattern_by_name branch and the now-dead
    write path (client.set_text_contents, the setTextContents DLL binding
    and FFI signature, the TextTooLong error variant). The advertisement
    gate and IsReadOnly are unchanged; the gating logic moves into a pure
    advertised_patterns() so it is unit-testable without a live bridge.
  - live fixture: the setTextContents round trip becomes the marker
    assertion. Its tree-wide "every advertised pattern has an instance"
    loop now asserts the inverse for capability markers, so the fixture
    guards the new rule instead of tolerating it.
  - Python surface is unaffected: get_pattern resolves proxies first, so
    patterns.TextEditable.set_text already synthesized keyboard input, and
    packages/native exposes no set-text binding. Docstrings say so now.

- **core:** Drop legacy inventory device-registration layer ([cbba6ad](https://github.com/imbus/robotframework-PlatynUI/commit/cbba6ad5d3fbf22c6477628ca62da9abc86bb80f))

  Removes the per-device / platform-module inventory registration and accessors
  left dead by the factory-only runtime (change per-runtime-platform-sessions).
  Zero production callers remained -- only the layer's own self-tests and a few
  mock non-registration tests.

  - core: deleted platform/registration.rs and platform/module.rs (the
    PlatformModule trait); removed the seven register_* macros, their
    *Registration structs + inventory::collect!, and the accessor fns
    (platform_modules / pointer_devices / keyboard_devices / screenshot_providers
    / highlight_providers / desktop_info_providers / window_managers /
    window_manager) plus their tests; pruned the mod.rs re-exports.
  - platform-mock: removed MockPlatform's vestigial PlatformModule impl and the
    '*_not_auto_registered' tests; kept all device-behavior coverage (a few mixed
    tests were narrowed rather than deleted).

  Inventory now backs only the two factory kinds (register_provider!,
  register_platform_factory!). No behavior change. Verified: just check, just test
  (2008 passed), just check-windows, just clippy-windows.

- **core:** Drop unused Technology marker ([e57a2b5](https://github.com/imbus/robotframework-PlatynUI/commit/e57a2b5769df60f260bdbc4f1cd4dd5a75bf6d1e))

  The Technology ABC and its sole UiNodeTechnology subclass added a
  filter dimension that no longer discriminates anything: with the
  single UiNode-backed adapter, framework_id already covers every
  plausible platform distinction. Removing it simplifies Adapter,
  AdapterProxy, WeightCalculator, and the pattern_proxy_for decorator,
  and drops the dead 100_000-point criterion.

- **core:** Remove Element.default_click_position in favour of ActivationTarget ([ea503de](https://github.com/imbus/robotframework-PlatynUI/commit/ea503de8244b41f6b90bdb67780c1397968256f4))

  Element now exposes only geometry/state (bounds, is_visible, is_in_view,
  is_enabled). The default click point lives on the MouseProxy; the
  AdapterMouseProxy fallback chain ends at Element.bounds.center() instead
  of an Element-level convenience property. Aligns Python with Rust
  (crates/core/src/ui/attributes.rs has no DefaultClickPosition attribute)
  and cleanly separates geometry from click-capability concerns.

  Designdoc bumped to Rev. 33; migration-status updated accordingly.

- **core:** Make AdapterCriteriaView public for cross-module reuse ([1c28e2a](https://github.com/imbus/robotframework-PlatynUI/commit/1c28e2a156747252ddb43f0fcba0541570fa6962))

  The Adapter-to-AdapterLike shim is needed by ContextFactory in
  context.py to drive WeightCalculator over an Adapter. Drop the
  leading underscore so the shim has a stable public name and no
  cross-module import has to reach into a private symbol.

- **core:** Adopt Runtime singleton in UiNodeAdapter + redesign Runtime API ([e68ef36](https://github.com/imbus/robotframework-PlatynUI/commit/e68ef36f54eb4a2f1a1da83b8966684b5b737afb))

  - UiNodeAdapter: drop runtime arg, use process-wide runtime.current
  - Runtime API v2: replace set/reset with use_default/use_mock/use_factory
    plus override/override_with_mock context managers; add sealing
  - PyO3 pattern API: replace .id() methods with pattern_name classattr on
    Focusable/WindowSurface; unify ancestor_pattern/top_level_pattern to
    accept str | type like has_pattern/get_pattern
  - Document the action/attribute split that motivates the native pattern
    wrappers in ui_node.py
  - Spec §A.5 rewritten; §A.8 lifecycle step 4 updated

- **core:** Align Rust PatternId with Python pattern_name (Reverse-DNS) ([09fdc6a](https://github.com/imbus/robotframework-PlatynUI/commit/09fdc6a19368fed296ee4ba9b94c3d334a9e3415))

  Switch all PatternId literals from bare names ("Focusable",
  "WindowSurface", …) to Reverse-DNS identifiers
  ("org.platynui.patterns.<Name>") so that Rust PatternId.as_str() and
  the upcoming Python PatternBase.pattern_name ClassVar match verbatim
  across the FFI boundary. This removes the need for a translation
  table and unblocks Phase 2 of the Python migration.

- **core:** Structure platform and provider boundary errors ([51a8e19](https://github.com/imbus/robotframework-PlatynUI/commit/51a8e190972e9d2eab7d8be2c38eb705ffcdf99d))

  Replace the old kind-based boundary error model with structured
  PlatformError and ProviderError variants.

  Keep English display text at the boundary, move dynamic context into
  named fields plus optional details, and document the new error-handling
  rules for future localization.

- **core:** Replace panics and println with proper error handling ([2c7a6ab](https://github.com/imbus/robotframework-PlatynUI/commit/2c7a6ab9718ae2b8693f19f8a402d7603a5516df))

  - Return Option from namespace prefix resolution instead of
    panicking on unknown input, preventing runtime crashes from
    invalid user-supplied prefixes
  - Surface unknown-namespace errors as Python ValueError at the
    FFI boundary for a clear diagnostic message
  - Replace all println! diagnostics in mock keyboard and pointer
    devices with structured tracing::debug! events, aligning with
    project logging conventions
  - Add descriptive messages to unreachable!() guards in the XPath
    predicate-pushdown optimizer for easier post-mortem diagnosis

- **core:** Tune keyboard timing defaults to ~240 WPM ([19f1797](https://github.com/imbus/robotframework-PlatynUI/commit/19f1797fa55a72a44efdf3b41fd135b9b8d79234))

  Adjust KeyboardSettings defaults to realistic values derived from
  keystroke dynamics research. The new timings (25 ms press, 5 ms release,
  20 ms between keys) yield ~240 WPM — fast enough for test automation
  while keeping events spaced for reliable application processing.

  Document default parameters and human reference profiles (50 WPM,
  120 WPM) in docs/keyboard-input.md with links to research sources.

- **core:** Remove strategies module and associated node traits ([530cdc4](https://github.com/imbus/robotframework-PlatynUI/commit/530cdc4ee56ff826404e8bab94eb3d2f1cee900a))
- **core,provider-uia:** Rename Application Name to ProcessName ([ad15649](https://github.com/imbus/robotframework-PlatynUI/commit/ad15649fcb0856c69cf7058f2d340b56efb13ec5))

  Rename `application::NAME` ("Name") to `application::PROCESS_NAME`
  ("ProcessName") to resolve a semantic collision: on AT-SPI2, the
  generic `control:Name` (from Accessible.Name) returns the application
  display name (e.g. "Firefox"), which conflicts with using the same
  attribute for the process executable stem.

  After this change:
  - `control:Name` = UI display name (inherited from common attributes)
  - `app:ProcessName` = executable filename without extension

  Windows UIA Application nodes now emit both `control:Name` (falls back
  to process name since UIA has no separate display name) and
  `app:ProcessName`. Also adds `control:Role` to the Application
  attribute iterator for consistency with regular UiaNode output.

  AT-SPI2 and Mock providers to be updated separately.

- **core,runtime:** Add #[must_use] to builder and value types ([54ecb83](https://github.com/imbus/robotframework-PlatynUI/commit/54ecb83d2292b2be50b7e73640917ca961658156))

  Annotate builder-pattern structs and geometric value types with
  #[must_use] so that discarding the return value of constructors or
  builder methods produces a compiler warning.

  Affected types: Rect, Size, ScrollDelta, KeyboardOverrides,
  HighlightRequest, MonitorInfo, WindowSurfaceActions, PointerProfile,
  PointerOverrides, EvaluateOptions.

  Point is intentionally excluded because it is used as the return type
  of side-effect functions (e.g. pointer_move_to) where ignoring the
  return value is legitimate.

- **docs:** Improve clarity in platform and provider descriptions ([08ad47d](https://github.com/imbus/robotframework-PlatynUI/commit/08ad47d288d57e07fdaec9f4957c47187b93e3bb))
- **egui:** Launch the test app from Robot Framework, not the session script ([0acda4c](https://github.com/imbus/robotframework-PlatynUI/commit/0acda4ccca66814c47f3bc5f5ebcef2134d1a145))

  The egui acceptance lane needs more than one window to exercise window
  activation, but the session script could only launch a single instance.
  Move the launch into Robot Framework so the suites decide how many windows
  exist and tear them down.

  - scripts/platynui-robot-session.sh: build platynui-test-app-egui and export
    its path as PLATYNUI_TEST_APP_BIN, but no longer launch it (the grace
    period and reap trap go with it).
  - tests/acceptance/egui/resources/testapp.resource: a Process-library wrapper
    — Launch Test App (distinct app id + title) waits for the window on the
    accessibility tree; Terminate App / Terminate Test Apps.
  - tests/acceptance/egui/__init__.robot: Suite Setup launches the default
    instance and Suite Teardown terminates it, so the existing smoke/query/
    interaction suites keep working unchanged.

- **evaluator:** Optimize set operations for streaming performance ([9676e14](https://github.com/imbus/robotframework-PlatynUI/commit/9676e144dbbe975f111a6506328cc8a898fb6b1f))
- **evaluator:** Replace attribute buffering with true streaming for improved performance ([29ad067](https://github.com/imbus/robotframework-PlatynUI/commit/29ad0675282a4d591bc16dd0930160eb9d3e6ed8))
- **focus:** Simplify focus command to handle single node evaluation ([2cacebe](https://github.com/imbus/robotframework-PlatynUI/commit/2cacebe554d76fa8259acf4acf2627d03845bb06))
- **inspector:** Adopt the workspace lints ([e11ad7b](https://github.com/imbus/robotframework-PlatynUI/commit/e11ad7bd6dd04ed76a30bd238b7a33bf001f009a))

  platynui-inspector never opted into `[workspace.lints]`, so `unsafe_code
  = deny`, `unused_crate_dependencies` and the pedantic set did not apply
  to it. It inherits them now; findings are fixed in code, and where the
  code is right as it is, the lint is allowed on the smallest item with
  the reason beside it.

  Test and mock-provider builds leave the OS provider crates unlinked on
  purpose, so the unused-dependency lint is allowed only for those builds.

- **inspector:** Harden async task handling ([a28ddee](https://github.com/imbus/robotframework-PlatynUI/commit/a28ddeef9967e3ba8072fe20da4ef426565b620d))

  Drain XPath search progress in bounded batches instead of keeping full
  result ownership in both the worker task and UI state. Add typed result
  status handling, epoch guards for reveal and highlight work, and validity
  checks for delayed node-backed actions.

  Update the Inspector documentation and improvement roadmap to reflect the
  new async hardening state and remaining follow-up work.

- **inspector:** Remove the root item count from the status bar ([38f0ea8](https://github.com/imbus/robotframework-PlatynUI/commit/38f0ea8d523570f2f8c7dec20786d7f4982d042c))
- **inspector:** Rename properties view to attributes ([44b03d1](https://github.com/imbus/robotframework-PlatynUI/commit/44b03d180456530fc3ed7a7601ef7bd5ec8a40f0))

  Align the inspector UI and module naming with the UiNode attribute
  terminology.

  Rename the former properties view and related symbols so code, docs,
  and user-facing labels consistently refer to attributes.

- **inspector:** Move automation work off UI thread and preload root tree ([53b37cc](https://github.com/imbus/robotframework-PlatynUI/commit/53b37cc6695dfee8c76291f8d11e7cb740136f22))

  Move UI automation calls to the inspector automation worker pool and
  stream initial root nodes into the tree cache.

  Preload root children before window creation to avoid startup stalls
  caused by expensive UIA root traversal after inspector registration.

  Switch tree child markers to cached child-presence data and keep
  refresh behavior synchronous and deterministic.

- **inspector:** Redesign TreeView as generic egui widget ([809f655](https://github.com/imbus/robotframework-PlatynUI/commit/809f65502ccc288bfedb527a5bf3c79d5a1134f8))

  Replace `show_tree()` with a reusable `TreeView<R: TreeRowData>` builder
  that encapsulates focus, keyboard navigation, hover, and click handling.

  - Single click handler on scroll area with hit-test-based row detection
  - Built-in keyboard nav via `set_focus_lock_filter` (moved from lib.rs)
  - Painted chevrons instead of unreliable Unicode characters
  - Constant 1px stroke to prevent layout shift on selection
  - Remove unused role icon infrastructure
  - Add horizontal scroll to properties table

- **inspector:** Migrate from Slint to egui ([f8b8b70](https://github.com/imbus/robotframework-PlatynUI/commit/f8b8b70fcafd4b5920b6a0bb05b36edba73f15c2))

  Replace the Slint-based inspector GUI with a pure Rust egui/eframe
  implementation using a clean MVVM architecture (Model/ViewModel/View).

- **inspector:** Simplify conditional checks for cached_bounds assignment ([f0ddb50](https://github.com/imbus/robotframework-PlatynUI/commit/f0ddb50bae04e5a6dcf7f580f756796b0f4b398e))
- **mock:** Make mock providers explicit-only, remove auto-registration ([65fbd53](https://github.com/imbus/robotframework-PlatynUI/commit/65fbd5369c2b7d2e07a1460bc236602eacf41fd5))
- **mock:** Modularize platform and provider scaffolds ([9d30cab](https://github.com/imbus/robotframework-PlatynUI/commit/9d30cab3ef57ddf06ee4b9d45ac96914a1171dd6))
- **mock-provider:** Drop provider-side geometry aliases ([26408e1](https://github.com/imbus/robotframework-PlatynUI/commit/26408e15b42ba1d787293ba8032f34d3eee66381))
- **native:** Adopt the workspace lints ([e7ab107](https://github.com/imbus/robotframework-PlatynUI/commit/e7ab1072262dd9565f4009db54a10d9db6b8d239))

  platynui_native never opted into `[workspace.lints]`, so `unsafe_code =
  deny`, `unused_crate_dependencies` and the pedantic set did not apply to
  it. It inherits them now; findings are fixed in code, and where the code
  is right as it is, the lint is allowed on the smallest item with the
  reason beside it.

  The Python API is unchanged: pyo3 signatures keep their shape, and the
  hash and argument conversions were checked against pyo3's own. Test and
  mock-provider builds leave the OS provider crates unlinked on purpose,
  so the unused-dependency lint is allowed only for those builds.

- **native:** Expose dict TypedDicts as public API ([7810900](https://github.com/imbus/robotframework-PlatynUI/commit/78109007ceccb531a4ce0ca559c4b7612639f9ff))

  Drop the underscore prefix from the nine TypedDict helpers used
  in the *Like type aliases (PointDict, SizeDict, SizeShortDict,
  RectDict, PointerOverridesDict, PointerSettingsDict,
  PointerProfileDict, KeyboardOverridesDict, KeyboardProfileDict)
  and add them to __all__.

  Robot Framework's libdoc cannot introspect PEP 695 'type X = ...'
  aliases but does understand TypedDict, so making the dict
  component visible lets keyword authors discover the accepted
  shape.

- **native:** Opt in pyo3 from_py_object ([6a784a8](https://github.com/imbus/robotframework-PlatynUI/commit/6a784a84c7d96e95b2cca12242edb913022fce56))

  PyO3 0.28 deprecates implicit FromPyObject for #[pyclass] + Clone
  mark Point/Size/Rect/IDs/Namespace with from_py_object to keep extraction behavior

- **parser:** Move grammar file path for XPathParser ([ed987e9](https://github.com/imbus/robotframework-PlatynUI/commit/ed987e99060ddda978c7ed8a9ae4ad52c74f9534))
- **pattern:** Enhance WindowSurfaceActions with user input handling and remove ApplicationStatus ([f4d3979](https://github.com/imbus/robotframework-PlatynUI/commit/f4d3979ae4d012c1872d253e9f2032531123dc5e))
- **patterns:** Split WindowSurfacePattern and align Python with Rust ([069e370](https://github.com/imbus/robotframework-PlatynUI/commit/069e370071d9cbb0ae867b2e7f1d1fd5d4381367))

  Splits the Rust mega-trait WindowSurfacePattern into seven orthogonal
  sub-traits (Activatable, Minimizable, Maximizable, Restorable,
  Closeable, Movable, Resizable) plus a Responsive polling pattern, and
  brings the Python pattern surface in line:

  - Rust: declare_action_pattern! macro for the five pure-action traits;
    MovablePattern/ResizablePattern carry typed payloads; ResponsivePattern
    wraps the previously inlined accepts_user_input probe.
  - Providers (atspi, mock, windows-uia) and the runtime/CLI consume the
    new traits directly; ACTIVATABLE doubles as the canonical top-level
    marker. Attribute table trimmed to the twelve modules actually wired
    up by Phase 1-4d.
  - PyO3: per-pattern wrapper classes replace PyWindowSurface; bindings
    expose Activatable/Minimizable/.../Responsive with matching stubs.
  - Python: rename HasUserInput -> Responsive (responsive.py) and drop
    Titled (Window.title now reads control:Name directly). Tests, helpers
    and BareMetal keywords follow suit.
  - Docs: python-library-design Rev. 37 reflects the new pattern names;
    python-migration-status marks Phase 4-rust-split DONE.

  Quality gates: cargo fmt/clippy/nextest (1981/1981), ruff/mypy/pyright
  clean, pytest 629/629.

- **patterns:** Drop window manager terminology ([766cf18](https://github.com/imbus/robotframework-PlatynUI/commit/766cf181c5ac2152796428777a645cdd609937f0))
- **platform-linux-wayland:** Remove dead code from connection module ([9843ee7](https://github.com/imbus/robotframework-PlatynUI/commit/9843ee741e82c191ea7145cdbdf00641d3c2e077))

  Remove unused `with_global()` function and the redundant `_conn` field
  from `WaylandGlobal`. The Wayland `Connection` (Arc-based) is now moved
  directly into the event loop thread instead of being cloned, eliminating
  an unnecessary reference count.

- **platform-linux-x11:** Adopt the workspace lints ([c874761](https://github.com/imbus/robotframework-PlatynUI/commit/c874761f0e5f9997f0816596de7ce50b0fcf5baf))

  platynui-platform-linux-x11 never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

  Its dependencies move into Linux-only tables like those of the other
  Linux platform crates, so other targets no longer build them for an
  empty crate.

- **platform-macos:** Adopt the workspace lints ([398612f](https://github.com/imbus/robotframework-PlatynUI/commit/398612f10bba13776c3d0e172759fda6be392dba))

  platynui-platform-macos never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

- **platform-mock:** Adopt the workspace lints ([e2f16e6](https://github.com/imbus/robotframework-PlatynUI/commit/e2f16e6e5d3a946d4340f913a0348aa7703078b1))

  platynui-platform-mock never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

  The synthetic screenshot keeps its saturating pixel casts on purpose.

- **platform-windows:** Adopt the workspace lints ([f9592d2](https://github.com/imbus/robotframework-PlatynUI/commit/f9592d28e9e225c31789c15866625d814675c929))

  platynui-platform-windows never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

  unsafe is allowed per Win32 FFI module, so the modules that make no OS
  calls keep the deny.

- **platform-windows:** Improve buffer handling in process query functions and enhance click-through overlay behavior ([6bac989](https://github.com/imbus/robotframework-PlatynUI/commit/6bac989a6d62aae5f2d0ef18d5d4b3c487680b27))
- **playground:** Adopt the workspace lints ([f921a4c](https://github.com/imbus/robotframework-PlatynUI/commit/f921a4c0f339b56897276e81119b2f8cb0cf970e))

  playground never opted into `[workspace.lints]`, so `unsafe_code =
  deny`, `unused_crate_dependencies` and the pedantic set did not apply to
  it. It inherits them now; findings are fixed in code, and where the code
  is right as it is, the lint is allowed on the smallest item with the
  reason beside it.

  The JAB spike is FFI throughout and allows unsafe file-wide. anyhow,
  serde and serde_json are dropped as unused.

- **pointer:** Update pointer click functions to accept optional target points ([bf725d6](https://github.com/imbus/robotframework-PlatynUI/commit/bf725d6430a57ba6b17f42aa5a10fcbe93523079))
- **pointer:** Reuse cached engine and clean overrides ([04dc88f](https://github.com/imbus/robotframework-PlatynUI/commit/04dc88fabbd1a694a2ade06ef3b3e5ac55ca825b))
- **pointer:** Simplify PointerSettings and PointerProfile initialization ([82ae325](https://github.com/imbus/robotframework-PlatynUI/commit/82ae325e5f7b9762bde85e83788ae2e4666ed3d3))
- **provide-windows-uia:** Streamline error handling with uia_api helper in COM interactions ([41fe022](https://github.com/imbus/robotframework-PlatynUI/commit/41fe0223c980fb4fe7c446df37e0fc076138e7fa))
- **provider:** Simplify lifecycle and add contract checks ([ad3f69c](https://github.com/imbus/robotframework-PlatynUI/commit/ad3f69c83209aefcca90aba0170edf031c237f99))
- **provider-atspi:** Adopt the workspace lints ([2496a09](https://github.com/imbus/robotframework-PlatynUI/commit/2496a09d9e8bfe45d5e9a7814847b8c5f7945d2e))

  platynui-provider-atspi never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

  libc becomes a Linux-only dependency; only the Linux process lookup uses
  it.

- **provider-atspi:** Stop exposing non-queryable element attributes ([be17929](https://github.com/imbus/robotframework-PlatynUI/commit/be1792923af87af7562d0ba56c085b593313f6c1))
  - **BREAKING**: stop exposing non-queryable element attributes

  Several AT-SPI attributes only ever carried a reference to another
  element (Parent, Application, RelationSet, a table's Caption/Summary, a
  cell's Table) or the Collection interface's ActiveDescendant. In
  PlatynUI you reach related elements by navigating XPath axes, not by
  matching an element's opaque identity in a predicate, so these
  attributes could never be used in a search and only cluttered the
  attribute set. They are no longer projected onto elements.

- **provider-atspi:** Replace /proc parsing with sysinfo and chrono ([88524ca](https://github.com/imbus/robotframework-PlatynUI/commit/88524caa123f019e9b17dfc7760650feb6e2b8f7))

  Replace manual /proc filesystem reads with the sysinfo crate for
  process metadata queries (exe path, command line, user ID, start time).
  This enables AT-SPI provider support on non-Linux Unix systems where
  /proc may be unavailable or structured differently.

  Replace hand-rolled calendar arithmetic for ISO 8601 timestamps with
  chrono::DateTime::from_timestamp().

  Keep libc::getpwuid_r for username resolution — sysinfo's Users API
  relies on getpwent enumeration which fails with SSSD/LDAP when
  enumerate=false.

- **provider-atspi:** Improve error handling, testing, and module structure ([4a66656](https://github.com/imbus/robotframework-PlatynUI/commit/4a66656562edc6240a841c6a1e11d5d58a5e8b65))

  - Add typed AtspiError enum (thiserror) replacing ad-hoc string errors
    in connection.rs, lib.rs, and node.rs with structured variants:
    ConnectionFailed, Timeout, DBus, ProxyUnavailable, InterfaceMissing,
    NoWindowManager, NodeDropped, FocusFailed
  - Implement From<AtspiError> for ProviderError and PatternError
  - Add UiNode::is_valid() override to detect zombie nodes via get_role()
  - Extract ClearableCell into dedicated clearable_cell module
  - Add 34 unit tests covering ClearableCell, normalize_value,
    pick_attr_value, map_role, map_role_with_interfaces, value
    conversions, and AtspiError conversions
  - Replace once_cell dependency with std::sync::{OnceLock, LazyLock}

- **provider-atspi:** Reduce fetch boilerplate with shared helpers ([c9d6dc3](https://github.com/imbus/robotframework-PlatynUI/commit/c9d6dc351053cd08fa3d41ad898e4a98ee7fa507))

  Extract recurring D-Bus fetch patterns into reusable helpers (fetch,
  fetch_str, fetch_map, fetch_int, fetch_string_map, geometry converters),
  eliminating ~160 lines of repetitive block_on_timeout_call chains across
  all 14 LazyNativeAttr fetch methods.

  Enrich Action.Actions with non-localized name from GetName(index),
  exposed as "Name" alongside "LocalizedName" from GetActions.

- **provider-atspi:** Consolidate block_on_timeout into shared module ([a6e2453](https://github.com/imbus/robotframework-PlatynUI/commit/a6e245342e1d9e39b693ca7f77dbb77ee6decb4a))

  Extract three duplicate block_on_timeout implementations (node.rs 1s,
  lib.rs 5s, connection.rs 10s) into a single parameterised function in a
  new timeout.rs module.

  The shared function adds consistent warn!-level logging with elapsed_ms
  and timeout_ms fields on every timeout, improving observability for the
  connection and init variants which previously logged nothing.

  Three convenience wrappers preserve ergonomics at call sites:
  - block_on_timeout_call (1s) — per-node D-Bus property reads
  - block_on_timeout_init (5s) — one-off provider startup calls
  - block_on_timeout_connect (10s) — a11y bus connection

- **provider-java:** Fold JAB into a single Java provider ([76c95dc](https://github.com/imbus/robotframework-PlatynUI/commit/76c95dc3987e70ef942a06923fe3342342f582a6))

  Java reaches PlatynUI through more than one channel — the Access Bridge for
  Swing/AWT today, an in-JVM agent next — and those channels do not cover the
  same windows. One provider per channel would have made two of them compete
  for the same top-level windows, forcing the window-claims registry from its
  boolean "claimed by someone else" question into rank-based ownership: new
  registry semantics plus every generic consumer, purely as a coordination
  protocol between two halves of one thing. Paying that was avoidable by not
  creating the second claimant at all.

  So there is now exactly one registered Java provider, and a channel is a
  backend of it. Claims stay boolean because there is only ever one Java
  claimant; gaining a backend changes which backend *serves* a window, never
  who *claims* it. `provider-java-swing` can drop its rank design entirely.

  - crates/provider-java: the router. It owns registration, configuration,
    the claims and backend selection; a backend owns its channel. Backend
    nodes travel unwrapped, so `@Technology`, the pattern set and
    `UiNode::is_valid` — load-bearing for scoped-root reuse — stay the
    backend's own answers rather than something the router restates.
  - crates/provider-java-jab: a library crate now, internals untouched. Its
    independent registration is gone and the claim owner id becomes `java`.
    Renamed from `provider-jab` because it no longer is a provider, and
    because the next backends want a family name to join. Keeping it a
    separate crate is what lets the router become portable when the in-JVM
    agent lands: JAB is Windows-only and stays behind a target-gated edge
    instead of dragging `libloading`, `sysinfo` and the `windows` features
    into a manifest that has to build everywhere.
  - The claim rule is "a backend can serve it", not "JAB can see it". With
    JAB as the only backend the claimed set is identical, but the general
    form is what keeps an SWT or JavaFX window — which JAB cannot reach at
    all, it speaks `javax.accessibility` — unclaimed and with the native
    provider instead of claimed and served empty.
  - The enablement diagnostic moves to the router. The backend still detects
    the Java-looking windows it cannot answer for (it needs `isJavaWindow`
    anyway) but reports them instead of warning: whether a window is truly
    unreachable is a question only the side that knows all backends can
    answer. Detection itself stays in the platform layer on purpose — it is
    the breadcrumb that must work precisely when no Java provider is active.

  The configuration follows: the three JAB keys move from `providers.jab.*`
  to `providers.java.jab.*` (`enabled`, `dll_path`, `call_timeout_ms`), with
  `providers.java.enabled` as the master switch and `providers.java.agent.*`
  reserved for the agent backend. The old spelling is gone rather than
  aliased or diagnosed — pre-1.0 with no public release, so there is no
  config anywhere that reads it, and the config layer ignores the section
  like any other unclaimed one.
  `providers.windows-uia.honor_window_claims` is unchanged.

  Behavior-preserving by construction, and that is the acceptance criterion:
  the Windows acceptance lane passes with zero changes to the Robot suites,
  and the crate-level tests changed only mechanically (config keys, claim
  owner id). The live fixture moved to the new crate because it drives the
  registered provider, which is now the umbrella; its assertions are
  unchanged.

- **provider-macos-ax:** Adopt the workspace lints ([a6acf7e](https://github.com/imbus/robotframework-PlatynUI/commit/a6acf7ede11803da10b49c497389a1e60e97f168))

  platynui-provider-macos-ax never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

- **provider-mock:** Adopt the workspace lints ([3b669d8](https://github.com/imbus/robotframework-PlatynUI/commit/3b669d8a21a7e6763d1e75a1ba2fb16414fc0bd4))

  platynui-provider-mock never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

  inventory is dropped: the crate stopped registering itself long ago.

- **provider-windows-uia:** Adopt the workspace lints ([dbf2f37](https://github.com/imbus/robotframework-PlatynUI/commit/dbf2f37b3e1c711897e438b2ab32588f17c73144))

  platynui-provider-windows-uia never opted into `[workspace.lints]`, so
  `unsafe_code = deny`, `unused_crate_dependencies` and the pedantic set
  did not apply to it. It inherits them now; findings are fixed in code,
  and where the code is right as it is, the lint is allowed on the
  smallest item with the reason beside it.

  Two reads of a header out of a byte buffer assumed an alignment that a
  Vec<u8> does not guarantee; they use read_unaligned now.

- **provider-windows-uia:** Stop exposing non-queryable element attributes ([dc9ab31](https://github.com/imbus/robotframework-PlatynUI/commit/dc9ab31c2fadd466446e06cd6177782990380936))

  UIA properties that hold references to other elements (ControllerFor,
  DescribedBy, FlowsTo/FlowsFrom, Selection, table header items, and other
  element arrays) surfaced as empty arrays that can never be populated or
  matched. Like single element references, they name elements to reach via
  XPath axes rather than values to match in a predicate, so they no longer
  appear as native attributes.

- **provider-windows-uia:** Cache app windows and harden COM init ([96305a9](https://github.com/imbus/robotframework-PlatynUI/commit/96305a94adf718eb06c5c94a3ad4b070c7e7dc6e))

  Preserve root streaming order by emitting desktop top-level nodes first and app:Application nodes afterwards.
  Build per-process window buckets during desktop traversal so Application children no longer rescan the root tree.
  Switch COM initialization to COINIT_MULTITHREADED and handle RPC_E_CHANGED_MODE explicitly, logging the expected mismatch at debug level.

- **provider-windows-uia:** Update COM initialization to use apartment-threaded model and remove warm-up code ([87eb9ef](https://github.com/imbus/robotframework-PlatynUI/commit/87eb9efa3d8ac0a88b3b6a54e67f770f120b49fc))
- **python:** Split adapter-aware device proxies into core.adapter_devices ([e16bfd6](https://github.com/imbus/robotframework-PlatynUI/commit/e16bfd619493adf5b6ad9e7790f62e283434d50e))

  Move AdapterMouseProxy and AdapterKeyboardProxy out of core.devices into
  the new core.adapter_devices module. core.devices now contains only the
  geometric, adapter-free layer (MouseProxy/KeyboardProxy, Anchor/
  VirtualPoint, action enums, settings bridge). Add MouseProxy.ctrl_click()
  that holds <Ctrl> via the runtime keyboard around a single click in a
  try/finally and replaces the former ctrl_click_adapter helper. Drop the
  ui/proxies/_mixins.py shim; default proxies (buttons, combobox, item,
  text) now instantiate AdapterMouseProxy/AdapterKeyboardProxy directly.
  Re-export path through PlatynUI.core stays unchanged. Replace the
  literal-required type-ignore comments in the settings-overrides bridge
  by direct literal-key assignments (using walrus). 724 tests green.

- **python:** Simplify assertable keywords handling in OurDynamicCore ([a677e29](https://github.com/imbus/robotframework-PlatynUI/commit/a677e29a20cba1b2fcfad53f8d0a707d212d9e13))
- **python-native:** Remove mock provider exports ([5dca925](https://github.com/imbus/robotframework-PlatynUI/commit/5dca9257899d9b3eee47556481acc279b94f61d5))
- **python-native:** Update pointer and keyboard overrides to use new type aliases ([75daeed](https://github.com/imbus/robotframework-PlatynUI/commit/75daeed6fdb42eb95985f40703553527dcbd934b))
- **python-native:** Rearrange type stubs ([c7c5a31](https://github.com/imbus/robotframework-PlatynUI/commit/c7c5a3160351256acaa21dd688b6c9d15abebe90))
- **python-native:** Simplify type annotations and improve consistency across the module ([5feda99](https://github.com/imbus/robotframework-PlatynUI/commit/5feda9936d289c1d9cc84ad2ee4a96a3213bef98))
- **runtime:** Adopt the workspace lints ([0132c57](https://github.com/imbus/robotframework-PlatynUI/commit/0132c576d196594d4cab5c9ef7317465de8af5c7))

  platynui-runtime never opted into `[workspace.lints]`, so `unsafe_code =
  deny`, `unused_crate_dependencies` and the pedantic set did not apply to
  it. It inherits them now; findings are fixed in code, and where the code
  is right as it is, the lint is allowed on the smallest item with the
  reason beside it.

  Public signatures stay as they are; where clippy would reshape one, the
  allow says why it cannot change.

- **runtime:** Split runtime.rs god file into directory module ([30b3216](https://github.com/imbus/robotframework-PlatynUI/commit/30b3216bf8cec3ea46db402a298c92c2cc6b2dc7))

  Replace the 2126-line runtime.rs with a runtime/ directory module
  containing 7 focused submodules:

- **runtime:** Unify duplicate sleep functions ([3bf50a0](https://github.com/imbus/robotframework-PlatynUI/commit/3bf50a08fdaa56dc0283bc8ba0559f1cb85fef89))

  Replace identical `default_pointer_sleep` and `default_keyboard_sleep`
  with a single `default_sleep` function. Both had the same body
  (skip if zero, otherwise thread::sleep). All 5 call sites updated.

- **runtime:** Rename keyboard settings to profile ([fdc5b0f](https://github.com/imbus/robotframework-PlatynUI/commit/fdc5b0fef3e3fbac76bb0d0d2a21d3281fe42015))

  Rename KeyboardSettings to KeyboardProfile across the runtime,
  bindings, and documentation.

  This makes the API clearer because the type describes keyboard timing,
  not general keyboard settings.

- **runtime:** Remove unused rect_alias_attributes function and related call in DesktopNode ([650a50e](https://github.com/imbus/robotframework-PlatynUI/commit/650a50e0bae9b1c77be78b39e5a25cff5ba0d5d7))
- **runtime:** Fold accepts user input into window surface ([78b2f7c](https://github.com/imbus/robotframework-PlatynUI/commit/78b2f7cd1888c067c125769083ea7bb95a2bdebf))
- **runtime:** Use ui node adapters for xpath ([c152ed8](https://github.com/imbus/robotframework-PlatynUI/commit/c152ed8f219600818754805a8c7647039a0afd92))
- **runtime,core:** Replace bare unwrap() on locks with expect() and Result propagation ([3aa6240](https://github.com/imbus/robotframework-PlatynUI/commit/3aa6240cc99a3b1f62ea3a394dc960a838187bd0))

  Replace Mutex/RwLock unwrap() calls across core and runtime crates:
  - Result-returning pointer methods: propagate new PointerError::Poisoned
  - Getters/setters and PatternRegistry: expect() with descriptive messages
  - Minor safe unwrap() in cli/platform crates: unwrap_or/unreachable

- **scripts:** Use X | None and single-quote strings in tools.py ([b932d69](https://github.com/imbus/robotframework-PlatynUI/commit/b932d695b6277cc564f2b4a832a524a93a8a47c3))

  Replace Optional[X] with the X | None syntax (PEP 604) and apply
  the project-wide single-quote convention enforced by ruff in the
  src/ tree.

- **scripts:** Clean up compositor session startup ([389711e](https://github.com/imbus/robotframework-PlatynUI/commit/389711e84f992d908c88547848891d539a8a1b60))

  - Fix session cleanup by removing exec from dbus-run-session and
    cargo run — the outer shell now survives to run its EXIT trap
    and remove the temporary XDG_RUNTIME_DIR
  - Add inner-script cleanup trap to kill background processes
    (at-spi-bus-launcher, registryd) on exit
  - Move all informational echo output to stderr to avoid mixing
    with program output on stdout
  - Extract AT-SPI bus setup into a reusable setup-atspi.sh helper
    that session scripts can source before launching applications
  - Remove AT-SPI setup from startcompositor.sh — the compositor
    itself has no AT-SPI dependency; accessibility infrastructure
    is a session-level concern
  - Remove xdg-desktop-portal setup — not needed for the compositor
    or test applications; can be added per session script if needed
  - Fix default log level comment (was "debug", actual default is
    "error")

- **settings,devices:** Align pointer/keyboard delay settings with native override schema ([922cb82](https://github.com/imbus/robotframework-PlatynUI/commit/922cb82249a62cfb64abc715d0c8e86003ae215f))

  Replace the legacy seconds-based mouse_*/keyboard_after_press_*/
  input_after_input_delay fields on Settings with millisecond fields
  that mirror platynui_native.PointerOverridesDict and
  KeyboardOverridesDict 1:1. Defaults are None so the active pointer/
  keyboard profile remains untouched unless a setting is explicitly set.

  MouseProxy/KeyboardProxy now build a per-call overrides dict from
  Settings.current() via two small table-driven helpers and pass it as
  the overrides= kwarg to every runtime pointer_*/keyboard_* call.
  Profile-level tuning (motion mode, acceleration, steps-per-pixel,
  scroll step etc.) stays exclusive to runtime.current.pointer_profile()
  / keyboard_profile() and is intentionally absent from Settings.

  Existing test_devices.py call assertions gain overrides=None; seven
  new bridge tests cover per-call resolution, nested Settings blocks,
  and propagation across move/press/release/click/multi_click/keyboard.

- **tests:** Simplify match statements in kind_tests_and_types ([f829a52](https://github.com/imbus/robotframework-PlatynUI/commit/f829a52d4b4795419a3ab26164b8089c873b986a))
- **wayland:** Eliminate production unwraps in input paths ([c8197b6](https://github.com/imbus/robotframework-PlatynUI/commit/c8197b66a4836f573cfab94fb97972ff9388d1fd))
- **wayland:** Centralize lints, gate to Linux, restructure docs ([32dc140](https://github.com/imbus/robotframework-PlatynUI/commit/32dc140f3061ee9110f7e7bc99473a82e16aa4b8))
- **wayland-compositor:** Remove dead_code allow for toplevel icon name field ([50442a9](https://github.com/imbus/robotframework-PlatynUI/commit/50442a95c4ad61fe2567f4b7fb4efb8c871ff109))

  Comment out the unused `name` field in `IconBuilder` and add TODO
  comments for future XDG icon theme lookup implementation.

- **xpath:** Adopt the workspace lints ([f200105](https://github.com/imbus/robotframework-PlatynUI/commit/f200105bdf551f6083190dd6c225da5496e56263))

  platynui-xpath never opted into `[workspace.lints]`, so `unsafe_code =
  deny`, `unused_crate_dependencies` and the pedantic set did not apply to
  it. It inherits them now; findings are fixed in code, and where the code
  is right as it is, the lint is allowed on the smallest item with the
  reason beside it.

  The integration tests now build as one binary under tests/it. As 175
  separate binaries each one saw every package dependency and would have
  needed its own unused_crate_dependencies allow; one binary also links
  once instead of 175 times. Every test keeps its name within its module.

- **xpath:** Eliminate unnecessary clones in context item callers ([0607844](https://github.com/imbus/robotframework-PlatynUI/commit/060784433d57fd3dacef02d1482dd1873d2010c5))

  Make require_context_item return &XdmItem instead of owned, and
  restructure all callers to work with references:

  - root_stream: match on &XdmItem, clone only the inner node for
    parent traversal; skip clone when node already is the root
  - base_uri_fn, document_uri_fn: match on &XdmItem, no clone needed
  - lang_fn: check start node by reference first, walk parents only
    via owned parent() calls
  - namespace_uri_stream: match on &XdmItem, no clone needed
  - number_default: inline to_number logic on &XdmItem, no clone
  - ids (id_fn, element_with_id_fn, idref_fn): pass Option<&XdmItem>
    into find_elements_with_id; topmost_ancestor_ref takes &N and
    clones only when the node itself is the root
  - Add doc comments to string_like_value / string_like_into_owned
    explaining borrow vs zero-copy move trade-off

- **xpath:** Streamline error handling in evaluation iterator ([922ff18](https://github.com/imbus/robotframework-PlatynUI/commit/922ff18bdafd6794f27b70dd84d283596e1f67c0))
- **xpath:** Simplify conditional checks using pattern matching ([bb04da6](https://github.com/imbus/robotframework-PlatynUI/commit/bb04da63fbfc1bfc0c47cdf6bc3fee1b7d5d06ba))
- **xpath:** Decompose evaluator into submodules and unify numeric enums ([a734cd4](https://github.com/imbus/robotframework-PlatynUI/commit/a734cd49dbe1b16f684e1abb38f77b27b7a76e3b))

  Split the monolithic 5,311-line evaluator.rs into 9 focused modules:
  - numeric.rs: unified NumKind + NumericKind (was duplicated in 3 places)
  - cursors.rs: all cursor types (axis, predicate, path, for-loop, etc.)
  - casting.rs: XSD type casting and temporal parsers
  - comparison.rs: atomic value comparison logic
  - set_ops.rs: set operations and document ordering
  - node_ops.rs: node test matching and name resolution
  - type_check.rs: instance-of type checking
  - xml_helpers.rs: XML string utility functions
  - mod.rs: public API, Vm types, and opcode dispatch

  Eliminates three independent definitions of numeric classification:
  - NumKind in execute() arithmetic block
  - NumKind in compare_atomic()
  - NumericKind in functions/common.rs
  All now consolidated in evaluator/numeric.rs.

- **xpath:** Add context messages to unreachable!() and replace unwrap() with expect() ([9083c88](https://github.com/imbus/robotframework-PlatynUI/commit/9083c884899fa72bb2ac846df066f6d83520c72d))
- **xpath:** Drop recursion lint allowance ([59188b8](https://github.com/imbus/robotframework-PlatynUI/commit/59188b8dc0aa542d9628371d40abe2060384a508))
- **xpath:** Enhance benchmarking with sampling mode and adjusted parameters ([09d06c7](https://github.com/imbus/robotframework-PlatynUI/commit/09d06c726e5bd37cbd4a47b36d441dab2f11d13d))
- **xpath:** Clean up code formatting and improve readability in various files ([adf60af](https://github.com/imbus/robotframework-PlatynUI/commit/adf60af566a33c5c9b60a57583a5d50b26fb527a))
- **xpath:** Replace `compile_xpath` with `compile` across various test files for consistency ([5f149e2](https://github.com/imbus/robotframework-PlatynUI/commit/5f149e2c51f003351246ad03d45c3278546a8d29))
- **xpath:** Replace children_vec with children iterator and remove redundant vector methods ([781b693](https://github.com/imbus/robotframework-PlatynUI/commit/781b69398044c4bf0f70d3e0e45da5dfc729c88c))
- **xpath:** Implement streaming evaluator with cursor-based architecture ([410627d](https://github.com/imbus/robotframework-PlatynUI/commit/410627d0586e788b72a25d0ce914ee894bf89395))
- **xpath:** Fix clippy warnings ([c38a9e8](https://github.com/imbus/robotframework-PlatynUI/commit/c38a9e85ddb9588cc67f96c53c89a91dcadfcb71))
- **xpath:** Reworked evaluator to be a streamed evaluator ([c9fa0fa](https://github.com/imbus/robotframework-PlatynUI/commit/c9fa0fa6fe6cf5e8055fa5629ada14b37be89f0d))
- **xpath:** Reduce context cloning and cache function registry ([6deb4f6](https://github.com/imbus/robotframework-PlatynUI/commit/6deb4f6f856f8cdccf590ffe72f87b9e845cb77b))
- **xpath:** Enhance error handling for context item requirements in id functions ([b6b7e1e](https://github.com/imbus/robotframework-PlatynUI/commit/b6b7e1e0b0ef4f662c5e82436dab4f07cbd2d7d3))
- **xpath:** Split default function registry into topic modules ([df5de62](https://github.com/imbus/robotframework-PlatynUI/commit/df5de62a39cebb46e645d627b993be6492a33b4a))
- **xpath:** Expand function registry and centralize namespace constants ([009e0e9](https://github.com/imbus/robotframework-PlatynUI/commit/009e0e922f0ff540489516a06ed00fddf4a9da47))
- **xpath:** Consolidation of error handling ([58915ff](https://github.com/imbus/robotframework-PlatynUI/commit/58915ff231ac4a9062ecf362e8dc94e2af75f589))
- **xpath:** Reorganize crate structure ([967fba6](https://github.com/imbus/robotframework-PlatynUI/commit/967fba64fccf832b6eccf82646a224c2eec74fa8))
- **xpath:** Simplify function implementations and improve readability in evaluator, functions, parser, and runtime modules ([a65678d](https://github.com/imbus/robotframework-PlatynUI/commit/a65678dcc2bf73bfff57b5d9ccf13a78acee22ea))
- **xpath,tests:** Simplify assertions and formatting in arithmetic and sum overflow tests ([6881e1c](https://github.com/imbus/robotframework-PlatynUI/commit/6881e1cd4f00f21f45f07c823eed8bacc55c3fe6))
- Resolve the findings from ruff's new default rules ([d9dd232](https://github.com/imbus/robotframework-PlatynUI/commit/d9dd232aecbe88583c60f34841d141cfb47e74a6))
  - **BREAKING**: pointer actions targeting a node that exposes neither
an ActivationPoint nor Bounds now raise TypeError instead of
ValueError.


  Mechanical cleanups plus three that change behaviour:

  - A node with neither an ActivationPoint nor Bounds now raises
    TypeError instead of ValueError (TRY004).
  - wait_for() carried an `except (PlatynUIFatalError, KeyboardInterrupt,
    SystemExit): raise` handler with no broader handler behind it, so it
    was a no-op. Removed, and the docstring corrected — the loop never
    caught anything, so every exception from a predicate propagates.
  - context.py now imports TracebackType at runtime instead of under
    TYPE_CHECKING, so __exit__ can be annotated without string literals.

  The rest: `raise e` -> `raise`, Self on Settings.__enter__, redundant
  string annotations, nested `with` statements, `None` moved to the end
  of a union, and needless shebangs in the native test modules.

  Deliberate patterns keep their behaviour and are marked with noqa:
  best-effort teardown against foreign UI processes (BLE001/S110), an
  immutable Rect default (B008), and a test that exercises __ne__ rather
  than __eq__ (SIM202).
- Remove unused PlatynUI.Types package ([f2aecff](https://github.com/imbus/robotframework-PlatynUI/commit/f2aecff337779fb46d383d311dfc53fef3a674a3))


  Legacy Title-Case re-export of Point/Rect/Size from platynui_native.
  Has zero importers in the new codebase, duplicates PlatynUI.core.types,
  and violates PEP 8 module naming. The same symbols remain available
  via PlatynUI.core.types.
- Drop reintroduced 'from __future__ import annotations' ([a7cde8b](https://github.com/imbus/robotframework-PlatynUI/commit/a7cde8b2043c44020aeabe92ff43d936eeae76c4))


  These imports slipped back in with the recent core/UI work. Per the
  established 3.12 floor (see 066e0d5) the future-import is no longer
  needed and removing it keeps annotations eager-evaluated as the
  introspection-based tooling expects.
- Adopt PEP 695 type alias syntax ([aca9e9e](https://github.com/imbus/robotframework-PlatynUI/commit/aca9e9e5592cb7efc4832f5cf494436504854957))


  Replace the legacy 'X: TypeAlias = ...' form with the lazy
  'type X = ...' statement (PEP 695). Forward references no longer
  need string quoting since type aliases are evaluated lazily.
- Import Self from typing instead of typing_extensions ([7919e9f](https://github.com/imbus/robotframework-PlatynUI/commit/7919e9f22421dd53ebdbfc1db4c7fc94ff5b0679))


  typing.Self has been available since 3.11; the typing_extensions
  fallback is no longer needed under the 3.12 minimum.
- Import Callable/Iterator from collections.abc ([ff6bf2b](https://github.com/imbus/robotframework-PlatynUI/commit/ff6bf2b1b7074f77545c41fb00f98d53dd743915))


  Aliases in 'typing' for collections.abc generics have been
  deprecated since 3.9 (PEP 585). Switch the two remaining call
  sites to the canonical import.
- Decorate inherited overrides with typing.override ([5ec49bb](https://github.com/imbus/robotframework-PlatynUI/commit/5ec49bb684c28345c8d542b414ec4cc4acd0781c))


  Apply @override (PEP 698, available since 3.12) to methods and
  properties that override base-class members in UiNodeAdapter,
  _NativeFocusable and AdapterMouseProxy. This lets type checkers catch
  silent breakage when base signatures change.
- Migrate to PEP 695 generic syntax ([25acc54](https://github.com/imbus/robotframework-PlatynUI/commit/25acc54a4e237f05f8144484125319f8f200ea40))


  All TypeVar usages were function- or method-scoped (not class-level
  Generic[T] bases). Inline declarations move the bound right next to the
  parameter that uses it, eliminating the module-level TypeVar boilerplate
  and the indirection of looking up a separate name.

  - core/adapter.py and core/adapter_proxy.py: 'P: PatternBase' inline on
    every get_pattern overload and the implementation; module-level
    TypeVar removed
  - core/adapter_proxy.py: 'ProxyT: AdapterProxy' inline on
    pattern_proxy_for; the inner decorator picks ProxyT up from the outer
    scope (PEP 695 makes the type parameter visible inside the function)
  - core/predicate.py: 'F: Callable[..., Any]' inline on predicate
  - packages/native/python/platynui_native/_native.pyi: '_P' inline on
    Node.get_pattern overload; the misplaced class-body TypeVar
    declaration is gone
- Drop 'from __future__ import annotations' under 3.12 floor ([066e0d5](https://github.com/imbus/robotframework-PlatynUI/commit/066e0d56660d220986b3a4148de2a738cd7770ac))


  The import was introduced for 3.10 compatibility (PEP 604 unions,
  built-in generics in annotations, lazy forward references). Under the
  3.12 floor it is no longer needed for syntax reasons; removing it makes
  annotations eager-evaluated again, which matches what runtime tooling
  that introspects annotations (dataclasses, pydantic, FastAPI, custom
  Adapter machinery, …) actually expects.
- Rename PatternId to PatternName for Rust/Python symmetry (Rev. 19) ([5eb7e18](https://github.com/imbus/robotframework-PlatynUI/commit/5eb7e18a5ac6a1935df686df1f21bac07e9788d3))


  The Rust UI traits used 'PatternId' while Python's PatternBase used
  'pattern_name'. Both refer to the same wire-level identifier
  (Reverse-DNS string, e.g. 'org.platynui.patterns.Invoke'). Aligning
  the names removes a long-standing source of confusion and keeps the
  two language bindings symmetric.

  Renames in Rust:
  - PatternId -> PatternName
  - UiPattern::id() -> pattern_name()
  - UiPattern::static_id() -> static_pattern_name()
  - UiNode::pattern_by_id() -> pattern_by_name()
  - pattern_ids() -> pattern_names()

  Wire format unchanged (still Reverse-DNS strings).

  PyO3 wrapper class is renamed to PatternName but intentionally not
  re-exported from platynui_native to avoid colliding with the Python
  TypeAlias PlatynUI.core.types.PatternName = str. Consumers needing
  the wrapper import from platynui_native._native.
- Clean up unnecessary clippy allows and raise argument threshold ([4ecb7c1](https://github.com/imbus/robotframework-PlatynUI/commit/4ecb7c1c961c610966910e0d03c6d839ad62ddfc))
- Improve error handling and remove dead code ([e9941a7](https://github.com/imbus/robotframework-PlatynUI/commit/e9941a7aa246e0f0c603c5339c5ff77c6e980e5f))


  - Return Option from namespace prefix resolution instead of
    panicking on unknown input, preventing runtime crashes from
    invalid user-supplied prefixes
  - Surface unknown-namespace errors as Python ValueError at the
    FFI boundary for a clear diagnostic message
  - Replace println diagnostics in mock keyboard and pointer
    devices with structured tracing events, aligning with project
    logging conventions
  - Add descriptive messages to unreachable guards in the XPath
    optimizer for easier post-mortem diagnosis
  - Remove unused builder methods, view-model helpers, and
    redundant data fields in the inspector
  - Remove superseded non-streaming XPath subsequence
    implementation fully replaced by the stream-based variant
- Replace once_cell with std equivalents across all crates ([6731fd6](https://github.com/imbus/robotframework-PlatynUI/commit/6731fd618e0eab929e69ffbcb91ee1913b2116c6))


  Migrate the entire workspace from `once_cell` to the standard library
  types `std::sync::LazyLock` (stable since 1.80) and
  `std::sync::OnceLock` (stable since 1.70), eliminating the external
  dependency from 9 crates.
- Remove Wayland dependencies and example for Linux in playground ([4fd7ef9](https://github.com/imbus/robotframework-PlatynUI/commit/4fd7ef99f464e2e65297b5b222d9e490e20dc8e5))
- Remove some uneeded unwraps ([48ce0d0](https://github.com/imbus/robotframework-PlatynUI/commit/48ce0d04be47a3b553f4cd8f41c731be51bc0e67))
- Simplify attribute value retrieval by combining conditions ([aba4561](https://github.com/imbus/robotframework-PlatynUI/commit/aba4561b4f5df1ac3fe3d4d039e41b59389cc436))
- Rename packages and update dependencies in project files ([346b346](https://github.com/imbus/robotframework-PlatynUI/commit/346b346bf74afa7831c83db143189419abfbde26))
- Replace Rc with Arc for runtime management in main function ([eb3ac97](https://github.com/imbus/robotframework-PlatynUI/commit/eb3ac97603aee8ebd4881334fa2d452f9b67b3bf))
- Unify highlight request handling and support multiple rectangles ([03bd173](https://github.com/imbus/robotframework-PlatynUI/commit/03bd173342286eb9926399409de635a641c661ca))
- Remove unnecessary target OS configuration for Windows ([eea7aa6](https://github.com/imbus/robotframework-PlatynUI/commit/eea7aa60f3f2ddc5b705f778929f7dedbc1b0b8e))
- Unify error handling (thiserror in libs, anyhow in CLI ([daec632](https://github.com/imbus/robotframework-PlatynUI/commit/daec63283be3d6b20f7b91a4df31936a2384523b))
- Improved readability and consistency ([357bd6a](https://github.com/imbus/robotframework-PlatynUI/commit/357bd6a001ec19df695f3fd78787aac757130158))
- Remove ProviderPriority from ProviderDescriptor and related code ([b0aa5b1](https://github.com/imbus/robotframework-PlatynUI/commit/b0aa5b19d2327c00f189b47461d6a62f67f65888))
- Reorder crates structure ([90a0d09](https://github.com/imbus/robotframework-PlatynUI/commit/90a0d09ab328880b107b52a5f89789f3382f4740))
- Rename XPath2Parser to XPathParser and update references in tests ([695bee1](https://github.com/imbus/robotframework-PlatynUI/commit/695bee11da9d029426cb21f31a048e1908bad3e5))


<!-- generated by git-cliff -->
