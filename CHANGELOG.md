# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

## [Unreleased]

### Bug Fixes

- **BareMetal:** Convert duration from seconds to milliseconds for highlight method ([9f3d1c7](https://github.com/imbus/robotframework-PlatynUI/commit/9f3d1c7f350ce9dcd100dd42becf1767bde52a95))
- **atspi:** Standardize namespace handling in AttrsIter ([a7b3827](https://github.com/imbus/robotframework-PlatynUI/commit/a7b382709a085a2596f4d6cb3b19ed5ad73553ab))
- **ci:** Add libinput and libgbm dependencies for Linux builds ([8d77718](https://github.com/imbus/robotframework-PlatynUI/commit/8d77718dcf3d6c362ec437450b4a207bfeed8c26))
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

- **dependencies:** Restrict robotframework-robocop version to <8.0.0 ([ffdbc7b](https://github.com/imbus/robotframework-PlatynUI/commit/ffdbc7b6039653e1ee235e1c4c6ee3947532e8b5))
- **evaluator:** Enhance boolean evaluation for node sequences ([3d8b85a](https://github.com/imbus/robotframework-PlatynUI/commit/3d8b85a81a9df54c7ab0fd5f43acb83fda8d6d61))
- **inspector:** Send window level command only on state change ([0038e98](https://github.com/imbus/robotframework-PlatynUI/commit/0038e98d73f33af1e2062db043c4dda8185d7d6a))

  The "Always On Top" viewport command was sent every frame, causing
  ~120 _NET_WM_STATE ClientMessages per second on X11/XWayland. This
  flooded the compositor log at DEBUG level and wasted resources.

  - Only send the WindowLevel viewport command when the always-on-top
    setting actually changes, not on every frame
  - Move the check after toolbar rendering so checkbox toggles take
    effect in the same frame

- **justfile:** Guard XDG_DATA_HOME against non-Linux platforms ([066fd4f](https://github.com/imbus/robotframework-PlatynUI/commit/066fd4f59ad5514b6c1856c0a6d7d50e59557933))

  The `env("HOME")` call fails on Windows where HOME is not set.
  Wrap the XDG path computation in `if os() == "linux"` so it
  short-circuits on other platforms, and add `[linux]` attributes
  to all desktop integration recipes.

- **native:** Remove ButtonLike from exports and add PointerButtonLike to __all__ ([b5bb344](https://github.com/imbus/robotframework-PlatynUI/commit/b5bb344d326e359b2531aa16248272e6bbaf8bf7))
- **native:** Simplify borrowing of PyDict in FromPyObject implementations for PointerOverridesInput and KeyboardOverridesInput ([9b47049](https://github.com/imbus/robotframework-PlatynUI/commit/9b470493fb754facd7a091170d2277223bb116cd))
- **platform-mock:** Remove auto-registration of mock highlight and keyboard devices ([e2ee2b9](https://github.com/imbus/robotframework-PlatynUI/commit/e2ee2b933b7b386d0890fa0e51c8b5e293ba405a))
- **platform-windows:** Enhance keyboard input handling with AltGr and modifier key aliases ([ea07d7f](https://github.com/imbus/robotframework-PlatynUI/commit/ea07d7fa63a66eeedf107f3b9727240a342dc7a5))
- **provider-windows-uia:** Replace Lazy with LazyLock in static descriptors ([ee7e508](https://github.com/imbus/robotframework-PlatynUI/commit/ee7e50864818f59301ddf703c78f92aa2c29a23c))

  The Linux refactoring missed two Lazy → LazyLock replacements in
  provider.rs, causing a build failure on Windows. The import for
  std::sync::LazyLock was already present.

- **provider-windows-uia:** Update UiaNode's availability check to use CurrentProcessId ([500b4a2](https://github.com/imbus/robotframework-PlatynUI/commit/500b4a2a8141e01af19f3b755c537bdb4ea3def0))
- **provider-windows-uia:** Correct getting clickable point and bounding rectangle ([1d4a42a](https://github.com/imbus/robotframework-PlatynUI/commit/1d4a42add4d3966cfd545512a2f884fd9adc1f1b))
- **python-bindings:** Restrict pointer origin to desktop|Point|Rect and update docs/README ([a5f2554](https://github.com/imbus/robotframework-PlatynUI/commit/a5f2554737e0085bf363378f5f0bfb5fdc9ae21c))

  - Remove tuple/dict acceptance for origin in platynui_native.runtime

  - Typing stays strict (runtime.pyi already limited)

  - Align docs and README example to core.Point instead of tuple

- **runtime:** Update FromPyObject implementations for PointerOverridesInput and KeyboardOverridesInput to match new PyO3 ([54e4bfc](https://github.com/imbus/robotframework-PlatynUI/commit/54e4bfc3057d069922ba43a1f095b5611bdcc031))
- **scripts:** Update regex pattern for robotframework-platynui version matching ([d0b0e41](https://github.com/imbus/robotframework-PlatynUI/commit/d0b0e414c56524a14abbdeb3e0dd87526a266467))
- **simple-node:** Assign document order for standalone trees ([4db05fd](https://github.com/imbus/robotframework-PlatynUI/commit/4db05fd6e5d1688f944a9896648d098ff2f3d191))
- **tests:** Update error message to use English for empty keyboard sequence ([e7a4100](https://github.com/imbus/robotframework-PlatynUI/commit/e7a41002768b80b83b139263d409d591918a09ee))
- **tests:** Add unused braces allowance for root and sc fixtures ([7170358](https://github.com/imbus/robotframework-PlatynUI/commit/717035832152f1fcd29363690e523878d6007fe3))
- **wayland:** Route pointer events to popups extending beyond SSD window bounds ([ac478fb](https://github.com/imbus/robotframework-PlatynUI/commit/ac478fb28c43332bccb254b02da3d420d7df9e53))

  SSD windows used only client geometry + decoration borders for hit-testing,
  so XDG popups (submenus, dropdowns) that extended beyond the parent window
  were visible but unreachable by the pointer.

  Now pointer_hit_test() also checks bbox_with_popups() via Space::element_bbox(),
  falling through to ClientArea when the cursor is over a popup outside the SSD
  zone. Fixes Kate and other Qt/GTK apps with cascading menus.

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

- **windows/highlight:** Check DC/DIB creation and clean up on failure ([d5a3413](https://github.com/imbus/robotframework-PlatynUI/commit/d5a34134b7aa70f4c652a9f33857ee1e6e3b3be3))

  - Guard  and  results; early return.
  - Replace  expect with safe match; free DCs on error paths.
  - Import /; no change to drawing logic.
  - Build and clippy stay clean (no warnings).

- **xkb-util:** Gate crate for Linux only to avoid Windows/macOS build failures ([f2b8090](https://github.com/imbus/robotframework-PlatynUI/commit/f2b8090ad4ed96da353518607e1be60bb476f956))

  xkbcommon is only available on Linux. Move dependency to
  `[target.'cfg(target_os = "linux")'.dependencies]` and wrap all
  public API re-exports with `#[cfg(target_os = "linux")]`.
  The crate compiles as empty on non-Linux targets.
  Consistent with eis-test-client which already gates its
  platynui-xkb-util dependency on Linux.

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
- Correct update version script ([f7b88a6](https://github.com/imbus/robotframework-PlatynUI/commit/f7b88a640a6d76132f2435b240e7c49a28806718))
- Add sync command to pre-bump hooks for all packages and extras ([e310385](https://github.com/imbus/robotframework-PlatynUI/commit/e310385a823e38851d922590b212e1363cbe5034))
- Add type hints and mypy configuration for improved type checking ([b31f326](https://github.com/imbus/robotframework-PlatynUI/commit/b31f32631908a82d79a6b5ed53a25b0769940a61))


### Documentation

- **api:** Implement evaluate API variant for flexible context handling ([93ebb28](https://github.com/imbus/robotframework-PlatynUI/commit/93ebb2885c3b3c06793340258323166897b67f0d))
- **copilot:** Correct terminology for packaging and update repository layout details ([886a7b2](https://github.com/imbus/robotframework-PlatynUI/commit/886a7b27eb45ad39bdcf8ac5ac76bc7e40708b0e))
- **core:** Align attribute names and architecture ([cb5579a](https://github.com/imbus/robotframework-PlatynUI/commit/cb5579a2583202687bc84407e1d04c2c46ec8632))
- **keyboard:** Refine runtime contract ([e6a3d9d](https://github.com/imbus/robotframework-PlatynUI/commit/e6a3d9dd3cf1f5af4d7c91a59734d4380cf05c43))
- **plan:** Update section headers and reorganize task lists in implementation plan ([88ec022](https://github.com/imbus/robotframework-PlatynUI/commit/88ec022c144f9420728b26d4af36a7980eed3167))
- **runtime:** Describe scroll_into_view action for control and item elements ([2bf4450](https://github.com/imbus/robotframework-PlatynUI/commit/2bf44504c886163ccc781985f95e9e8c92442b4f))
- **runtime:** Rename implementation plan for PlatynUI Runtime ([4dd42dd](https://github.com/imbus/robotframework-PlatynUI/commit/4dd42dd9c3fd4100af981c1603ae74e7cbad6612))
- **runtime:** Clarify control vs app namespace usage ([5746ce1](https://github.com/imbus/robotframework-PlatynUI/commit/5746ce10fd7b5594d52347539aac4bceffc082d5))
- **runtime:** Clarify WindowSurface guidance ([17ab132](https://github.com/imbus/robotframework-PlatynUI/commit/17ab1320fcc9dc4a1973a3c396a33aa2d4ed7488))
- **windows:** Note dpi-aware pointer task ([7b1976e](https://github.com/imbus/robotframework-PlatynUI/commit/7b1976e2e854fda73e4c753cb9bc27b3b1e6fa1d))
- **xpath:** Update XPath 2.0 coverage notes for compatibility mode and namespace tracking ([b80aef7](https://github.com/imbus/robotframework-PlatynUI/commit/b80aef77981d9df44c6050ecbd37b76d1ea189d8))
- **xpath:** Add a Readme and comprehensive coverage documentation for XPath 2.0 compliance ([3a0920e](https://github.com/imbus/robotframework-PlatynUI/commit/3a0920eb99387fa8ccaa41b8042ee35840ffd1f2))
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
- **atspi:** Add EWMH helpers for X11 window management and integrate WindowSurface pattern ([c4903b0](https://github.com/imbus/robotframework-PlatynUI/commit/c4903b07a4c5ce8af20868a4fc8590b3313f4394))
- **atspi:** Add process ID resolution for application nodes ([babe870](https://github.com/imbus/robotframework-PlatynUI/commit/babe8704c84b8d93208593661b257bed286edcfe))
- **cli:** Add global no-activate option to prevent window activation before actions ([ac00509](https://github.com/imbus/robotframework-PlatynUI/commit/ac0050969fadabf47c5a979885e420e2d7a65a9a))
- **cli:** Add snapshot command for exporting UI subtrees as XML ([60bf6b0](https://github.com/imbus/robotframework-PlatynUI/commit/60bf6b06dd2e0a7f6822fa803d452dc4aedb2051))
- **cli:** Enhance pointer commands to return element information on actions ([fde9ed3](https://github.com/imbus/robotframework-PlatynUI/commit/fde9ed394d2beef4d28d4768689dee8a54fafe2b))
- **cli:** Enhance pointer commands to support XPath expressions and optional point arguments ([2b8a8ae](https://github.com/imbus/robotframework-PlatynUI/commit/2b8a8aedf347bd1e5a0df2464e9ba4219efefc52))
- **cli:** Add focus command leveraging runtime patterns ([504a529](https://github.com/imbus/robotframework-PlatynUI/commit/504a529baf59f475c68bc8e91a07dba044599eb4))
- **cli:** Colorize query output ([704d3c4](https://github.com/imbus/robotframework-PlatynUI/commit/704d3c433e0a5ed865a18187f0e4afb86bb4c9c4))
- **cli:** Add watch command for provider events ([273b42d](https://github.com/imbus/robotframework-PlatynUI/commit/273b42df4708f72ea721b3cae44bcd55b7931084))
- **cli:** Add mock-backed provider listing ([bbaef5f](https://github.com/imbus/robotframework-PlatynUI/commit/bbaef5f1093588ba77fdcb7731622033d5c46ebc))
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
- **mock:** Render platynui testcard screenshot ([c980306](https://github.com/imbus/robotframework-PlatynUI/commit/c980306a2c433188f9339581ea84d7b4b1b51dbd))
- **mock:** Load mock tree from xml and improve query output ([87b83df](https://github.com/imbus/robotframework-PlatynUI/commit/87b83dfaffa2c793faf60dcb5990a5f4b33e4176))
- **optimizer:** Add constant folding optimization and related benchmarks ([a2e5f43](https://github.com/imbus/robotframework-PlatynUI/commit/a2e5f43ada82ae98acaa51b83273e341760b0bc2))
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

- **platform-linux-x11:** Implement X11 keyboard device ([72d7d73](https://github.com/imbus/robotframework-PlatynUI/commit/72d7d731bb5f464b46fe5b223f0152d62927a7db))

  XTest-based keyboard injection with keysym-to-keycode resolution
  via GetKeyboardMapping. Supports named keys (~120 entries),
  single-character input with CapsLock-aware shift management, and
  dynamic keycode remapping for characters outside the active layout.

  Control characters (\n, \t, \r, etc.) are mapped to their
  corresponding X11 TTY function keysyms (Return, Tab, etc.).

- **platform-linux-x11:** PlatformModule initialization with eager connection and extension checks ([fd3ffca](https://github.com/imbus/robotframework-PlatynUI/commit/fd3ffcac853fb5cd16fbfaed24a14e2d43812fa2))
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
- **provide-windows-uia:** Enhance virtualized item handling and improve parent-child relationships ([d56105d](https://github.com/imbus/robotframework-PlatynUI/commit/d56105de2f92cba71a031aeeaadf88b7ddbc6406))
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

- **provider-uia:** Add support for window state attributes (IsMinimized, IsMaximized, IsTopmost) and user input acceptance ([b9a45ef](https://github.com/imbus/robotframework-PlatynUI/commit/b9a45ef43296f25792e6dc65fe04ec0528f01d3d))
- **provider-window-uia:** Implement WaitForInputIdleChecker ([6927795](https://github.com/imbus/robotframework-PlatynUI/commit/6927795b54aaee0aec2b784bd07b0bf7e3966790))
- **provider-windows-uia:** Implement scoped RuntimeId URIs for UiaNode and related attributes ([c7c4623](https://github.com/imbus/robotframework-PlatynUI/commit/c7c462327b3084199a5c21f5645cda04c3716a5c))
- **provider-windows-uia:** Implement Application view ([c51a3f4](https://github.com/imbus/robotframework-PlatynUI/commit/c51a3f459b12e9082ed8f11df3fc171ea71a93ed))
- **provider-windows-uia:** Implement native property support ([b262755](https://github.com/imbus/robotframework-PlatynUI/commit/b26275595166ffcc1a78e62245ba2dafea48a99d))
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

- **ui:** Add optional developer-provided stable identifier `Id` to UiNode and related attributes ([8ee7905](https://github.com/imbus/robotframework-PlatynUI/commit/8ee7905f1dcf12c9117f2f2dd8922d753dbf15a5))
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
- **cli:** Simplify attribute handling in pointer commands and improve iterator type definitions in XPath ([58f7814](https://github.com/imbus/robotframework-PlatynUI/commit/58f781416754fbfe2c294d9389ab57d06f844660))
- **cli:** Remove unused filters ([1abd0e8](https://github.com/imbus/robotframework-PlatynUI/commit/1abd0e8fb6dd6bca4f73c87058dbccafb1f22e9c))
- **cli:** Split subcommands into modules ([0927dc9](https://github.com/imbus/robotframework-PlatynUI/commit/0927dc942585fcc3883c209174ebc65fee9c27e4))
- **cli,platform-mock:** Unify CLI output; move logs to mocks ([9fb5817](https://github.com/imbus/robotframework-PlatynUI/commit/9fb5817971200b0ad6e795d898632c11342b8901))
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

- **docs:** Improve clarity in platform and provider descriptions ([08ad47d](https://github.com/imbus/robotframework-PlatynUI/commit/08ad47d288d57e07fdaec9f4957c47187b93e3bb))
- **evaluator:** Optimize set operations for streaming performance ([9676e14](https://github.com/imbus/robotframework-PlatynUI/commit/9676e144dbbe975f111a6506328cc8a898fb6b1f))
- **evaluator:** Replace attribute buffering with true streaming for improved performance ([29ad067](https://github.com/imbus/robotframework-PlatynUI/commit/29ad0675282a4d591bc16dd0930160eb9d3e6ed8))
- **focus:** Simplify focus command to handle single node evaluation ([2cacebe](https://github.com/imbus/robotframework-PlatynUI/commit/2cacebe554d76fa8259acf4acf2627d03845bb06))
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
- **native:** Opt in pyo3 from_py_object ([6a784a8](https://github.com/imbus/robotframework-PlatynUI/commit/6a784a84c7d96e95b2cca12242edb913022fce56))

  PyO3 0.28 deprecates implicit FromPyObject for #[pyclass] + Clone
  mark Point/Size/Rect/IDs/Namespace with from_py_object to keep extraction behavior

- **parser:** Move grammar file path for XPathParser ([ed987e9](https://github.com/imbus/robotframework-PlatynUI/commit/ed987e99060ddda978c7ed8a9ae4ad52c74f9534))
- **pattern:** Enhance WindowSurfaceActions with user input handling and remove ApplicationStatus ([f4d3979](https://github.com/imbus/robotframework-PlatynUI/commit/f4d3979ae4d012c1872d253e9f2032531123dc5e))
- **patterns:** Drop window manager terminology ([766cf18](https://github.com/imbus/robotframework-PlatynUI/commit/766cf181c5ac2152796428777a645cdd609937f0))
- **platform-windows:** Improve buffer handling in process query functions and enhance click-through overlay behavior ([6bac989](https://github.com/imbus/robotframework-PlatynUI/commit/6bac989a6d62aae5f2d0ef18d5d4b3c487680b27))
- **pointer:** Update pointer click functions to accept optional target points ([bf725d6](https://github.com/imbus/robotframework-PlatynUI/commit/bf725d6430a57ba6b17f42aa5a10fcbe93523079))
- **pointer:** Reuse cached engine and clean overrides ([04dc88f](https://github.com/imbus/robotframework-PlatynUI/commit/04dc88fabbd1a694a2ade06ef3b3e5ac55ca825b))
- **pointer:** Simplify PointerSettings and PointerProfile initialization ([82ae325](https://github.com/imbus/robotframework-PlatynUI/commit/82ae325e5f7b9762bde85e83788ae2e4666ed3d3))
- **provide-windows-uia:** Streamline error handling with uia_api helper in COM interactions ([41fe022](https://github.com/imbus/robotframework-PlatynUI/commit/41fe0223c980fb4fe7c446df37e0fc076138e7fa))
- **provider:** Simplify lifecycle and add contract checks ([ad3f69c](https://github.com/imbus/robotframework-PlatynUI/commit/ad3f69c83209aefcca90aba0170edf031c237f99))
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

- **provider-windows-uia:** Update COM initialization to use apartment-threaded model and remove warm-up code ([87eb9ef](https://github.com/imbus/robotframework-PlatynUI/commit/87eb9efa3d8ac0a88b3b6a54e67f770f120b49fc))
- **python:** Simplify assertable keywords handling in OurDynamicCore ([a677e29](https://github.com/imbus/robotframework-PlatynUI/commit/a677e29a20cba1b2fcfad53f8d0a707d212d9e13))
- **python-native:** Remove mock provider exports ([5dca925](https://github.com/imbus/robotframework-PlatynUI/commit/5dca9257899d9b3eee47556481acc279b94f61d5))
- **python-native:** Update pointer and keyboard overrides to use new type aliases ([75daeed](https://github.com/imbus/robotframework-PlatynUI/commit/75daeed6fdb42eb95985f40703553527dcbd934b))
- **python-native:** Rearrange type stubs ([c7c5a31](https://github.com/imbus/robotframework-PlatynUI/commit/c7c5a3160351256acaa21dd688b6c9d15abebe90))
- **python-native:** Simplify type annotations and improve consistency across the module ([5feda99](https://github.com/imbus/robotframework-PlatynUI/commit/5feda9936d289c1d9cc84ad2ee4a96a3213bef98))
- **runtime:** Remove unused rect_alias_attributes function and related call in DesktopNode ([650a50e](https://github.com/imbus/robotframework-PlatynUI/commit/650a50e0bae9b1c77be78b39e5a25cff5ba0d5d7))
- **runtime:** Fold accepts user input into window surface ([78b2f7c](https://github.com/imbus/robotframework-PlatynUI/commit/78b2f7cd1888c067c125769083ea7bb95a2bdebf))
- **runtime:** Use ui node adapters for xpath ([c152ed8](https://github.com/imbus/robotframework-PlatynUI/commit/c152ed8f219600818754805a8c7647039a0afd92))
- **runtime,core:** Replace bare unwrap() on locks with expect() and Result propagation ([3aa6240](https://github.com/imbus/robotframework-PlatynUI/commit/3aa6240cc99a3b1f62ea3a394dc960a838187bd0))

  Replace Mutex/RwLock unwrap() calls across core and runtime crates:
  - Result-returning pointer methods: propagate new PointerError::Poisoned
  - Getters/setters and PatternRegistry: expect() with descriptive messages
  - Minor safe unwrap() in cli/platform crates: unwrap_or/unreachable

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

- **tests:** Simplify match statements in kind_tests_and_types ([f829a52](https://github.com/imbus/robotframework-PlatynUI/commit/f829a52d4b4795419a3ab26164b8089c873b986a))
- **wayland:** Centralize lints, gate to Linux, restructure docs ([32dc140](https://github.com/imbus/robotframework-PlatynUI/commit/32dc140f3061ee9110f7e7bc99473a82e16aa4b8))
- **wayland-compositor:** Remove dead_code allow for toplevel icon name field ([50442a9](https://github.com/imbus/robotframework-PlatynUI/commit/50442a95c4ad61fe2567f4b7fb4efb8c871ff109))

  Comment out the unused `name` field in `IconBuilder` and add TODO
  comments for future XDG icon theme lookup implementation.

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
