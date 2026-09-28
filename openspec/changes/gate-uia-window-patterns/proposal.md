# Proposal

## Why

The UI Automation provider advertises the eight window capability patterns (Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable, Responsive) only on elements with a UIA WindowPattern or TransformPattern. It hands out an instance of each of them for every element, though (`crates/provider-windows-uia/src/node.rs:503-524` versus `:560-568`). The runtime looks for "the element's window" through those instances (`crates/runtime/src/runtime/window.rs:44-58`), so on Windows every element is its own window. Activation of that "window" has been a UIA `SetFocus` on the element since October 2025 (`node.rs:588-636`). That has these effects:

- Highlight, Take Screenshot and Pointer Move To move the keyboard focus inside the application to their target, which fires the application's focus handlers.
- Bring To Front on a disabled element fails, because UIA refuses to focus it. The implicit activation before a click on such an element fails silently, and the window stays behind.
- For a native control with its own window handle, activation waits up to one second for a foreground change it can never observe.
- A minimized window is not restored when an element inside it is activated.
- It hid a defect: a held element that had lost its ancestors still raised its window. `xdm-snapshot-release` had to work around this in a test.

Nothing detects the mismatch. The contract testkit only reads `supported_patterns()`, and `validate_control_or_item` claims to check pattern instances but does not.

## What Changes

- **UIA hands out window patterns only where it advertises them.** `pattern_by_name` gets the same gate as `supported_patterns`, as `Focusable` already has. The element's window is then found through its ancestors, which providers keep alive since `xdm-snapshot-release`.
- **UIA activates windows through the runtime's window manager, like JAB and the Java agent do.** The provider takes the window manager the runtime already injects into every provider and currently drops. Activation resolves the top-level (root) window of the element's native window handle, or of its nearest ancestor that has one. It restores that window if it is minimized, and makes it the foreground window with the window manager's foreground-lock workaround. UIA `SetFocus` remains only for an element in whose chain no native window handle exists. Without a window manager, activation fails with an error that says so, as it already does for Java and AT-SPI windows.
- **Activation requests no focus change inside the window.** This is a rule for every provider and is written into `window-activation`. Bring To Front no longer depends on the element being enabled or focusable.
- **A testkit check proves the contract for every provider.** For each core action pattern, an instance exists exactly when the pattern is advertised, and the TextEditable marker never has one. It runs against the mock (CI), the UIA test window, and the Java backends (live tests). The documentation of `validate_control_or_item` is corrected to what the function checks.
- **Smaller parts:**
  - the WindowPattern and TransformPattern availability is read with the enumeration cache;
  - `has_window_surface` short-circuits;
  - the Win32 window manager's record of a refused foreground change stays at debug, as `4b1fc6c` set it and the `diagnostic-logging` spec asks. UIA activation now reaches it before every pointer and keyboard action, and debug records do not burden the lane.
- **Deliberately unchanged:** Minimize, Maximize, Restore, Close, Move and Resize keep using UIA's WindowPattern and TransformPattern. The finer split of the gate (window operations only with WindowPattern, Move and Resize only with TransformPattern) is left for a follow-up. The mock provider stays as it is.

Behavior changes that users see, for the release notes:

- On Windows, Highlight, Take Screenshot and the pointer keywords no longer move the keyboard focus inside the target window.
- Bring To Front, Activate Window and the implicit activation raise the element's top-level window. That includes disabled elements, and a minimized window is restored.
- Bring To Front on an element with no window above it (some popup menus and tooltips) now fails with the missing-Activatable error that names the element, instead of focusing it.
- `platynui-cli window` skips non-window elements as "missing Activatable".
- A providers-only runtime (no platform) can no longer activate UIA windows.

## Capabilities

### New Capabilities

- `uia-window-activation`: how the UI Automation provider activates a window: through the runtime's window manager, on the root window of the element's native window handle, with the precedence between the window-manager path, the UIA focus fallback and the missing-window-manager error.
- `pattern-advertisement`: the agreement between the action patterns a node advertises and the pattern instances it hands out, for every provider, and the contract testkit check that proves it.

### Modified Capabilities

- `uia-common-attributes`: ADDED requirement that the window capability patterns are advertised and served only on elements with a window surface, next to the existing Focusable gate.
- `window-activation`: ADDED requirements that activation requests no focus change inside the window and does not depend on the element accepting focus. The capability's main spec exists since `window-activation-state` was archived (2026-09-28).

## Impact

- **Rust crates:**
  - `crates/provider-windows-uia`:
    - `src/provider.rs`: `set_window_manager`, set-once, and threading the manager into the nodes;
    - `src/node.rs`: the `pattern_by_name` gate, activation through the window manager, the short-circuit in `has_window_surface`, the test window tests;
    - `src/com.rs`: the cache request;
    - `Cargo.toml`: `platynui-platform-windows` as a dev-dependency for the ignored activation test.
  - `crates/core`: `verify_pattern_instances` and new `ContractIssue` variants in `src/ui/contract/testkit.rs`, a single list of the action pattern names, and the corrected documentation of `validate_control_or_item`.
  - `crates/platform-windows`: only comments are corrected (`src/window_manager.rs`, the property id of `NativeWindowHandle`). No change to how it resolves or activates windows, or to how it logs a refusal.
  - `crates/provider-mock`: a conformance test only. `crates/provider-java`: the live tests add the new check next to their own.
- **Python/RF:** no API change. The docs of Activate Window and Bring To Front and the section "Bringing windows to the front" now say that the window becomes the active window and that PlatynUI moves no focus inside it (`src/PlatynUI/BareMetal/__init__.py`).
- **Tests:**
  - read-only UIA tests on the test window of `xdm-snapshot-release`, whose window is the positive case and its buttons the negative one;
  - an ignored activation test with the real Win32 window manager, which the Windows lane runs;
  - core and mock tests;
  - egui acceptance scenarios on every lane.

  No test uses the taskbar, the shell or applications that Windows ships.
- **Native rebuild:** yes.
- **Platforms:** Windows (UI Automation) changes. AT-SPI, JAB and the Java agent already gate their window patterns and activate through the window manager; they only gain the conformance check.
- **Docs:** `dev-docs/platform-windows.md`, `dev-docs/architecture.md` (the UIA activation row, the layer diagram that already names the window manager, the pattern-honesty rule), `dev-docs/testing-strategy.md` (the new testkit check).
- **Coordination:**
  - `window-activation-state` (archived 2026-09-28) changed the UIA activation code this change replaces. This change reverses its decision D4 against injecting the window manager into UIA.
  - `snapshot-validity` and `application-process-attributes` edit the same `ApplicationNode` code, so the changes land one after another.
