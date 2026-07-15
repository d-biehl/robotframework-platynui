# Windows Platform Implementation

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the Windows-specific implementation details for PlatynUI: platform devices, UIA provider, and Win32 WindowManager. For the platform-agnostic architecture, see `dev-docs/architecture.md`.

## 1. Platform Devices

**Initialization** (`PlatformModule::initialize()`):
- Sets `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` before any device or provider initialization.
- Coordinates throughout use Desktop pixels (Virtual Screen).

**Desktop & Monitors**:
- Desktop bounds from Virtual Screen (`SM_*VIRTUALSCREEN`).
- Monitors via `EnumDisplayMonitors` + `GetMonitorInfoW(MONITORINFOEXW)`. Friendly names via `DisplayConfigGetDeviceInfo`.
- OS version as `<major>.<minor>[.<build>]`.
- DPI/scale per monitor: `GetDpiForMonitor(MDT_EFFECTIVE_DPI)` → `scale_factor = dpi/96.0`.

**Pointer**: `SendInput` API. Desktop-absolute coordinates.

**Keyboard**: `SendInput` + `VkKeyScanW` for character mapping.
- Complete VK name map (without `VK_` prefix): `ESCAPE`, `RETURN`, `F24`, `LCTRL`, `RMENU`, etc.
- Left/right modifier aliases: `LSHIFT`/`LEFTSHIFT`, `RSHIFT`/`RIGHTSHIFT`, `LCTRL`/`LEFTCTRL`, `RCTRL`/`RIGHTCTRL`, `ALTGR`/`RALT`/`RIGHTALT`, `LEFTWIN`/`RIGHTWIN`.
- Symbol aliases: `PLUS`, `MINUS`, `LESS`/`LT`, `GREATER`/`GT`.
- AltGr: when `VkKeyScanW` signals `Ctrl+Alt`, injects `VK_RMENU` (Right Alt) instead.
- Extended keys: `KEYEVENTF_EXTENDEDKEY` set for Right Ctrl/Alt, navigation keys, NumLock, etc.
- Fallback: Unicode injection (`KEYEVENTF_UNICODE`) for unmappable characters.
- CapsLock: shift bit inverted for letters when CapsLock is active.

**Screenshot**: GDI `CreateDIBSection` (top-down, 32 bpp) + `BitBlt`. Returns `BGRA8`. Region clamped to Virtual Screen bounds.

**Highlight**: Layered window (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE`). Non-activating, click-through, not in Alt-Tab/Taskbar. Red frame (3px, RGBA 255,0,0,230) with 1px padding. Clipped sides drawn dashed (6 on / 4 off). Auto-clear via generation-aware timer.

## 2. UIA Provider

**Threading & COM**:
- One-time `CoInitializeEx(..., COINIT_MULTITHREADED)` per thread.
- Thread-local singletons: `com::uia()` (`IUIAutomation`) and `com::raw_walker()` (`IUIAutomationTreeWalker`).
- No separate actor thread.

**Tree Traversal**:
- Exclusive use of Raw View TreeWalker (`GetFirstChildElement`, `GetNextSiblingElement`, `GetParentElement`).
- No `FindAll`. A traversal `CacheRequest` (`com::traversal_cache_request()`) is attached to walkers to batch property reads during traversal.
- Lazy iterator implementation (`ElementChildrenIter`) with first-flag and sibling traversal.

**Node Model** (`UiaNode`):
- Wraps `IUIAutomationElement` directly (no intermediate store).
- Lazy attribute evaluation — `UiAttribute::value()` reads from UIA on demand.
- `invalidate()` is a no-op (attributes are always lazily re-read).

**Attributes**:
- `Role`: from ControlType → normalized PascalCase
- `Name`: from `CurrentName()`
- `RuntimeId`: from `GetRuntimeId()` → scoped URI (`uia://desktop/<hex>` or `uia://app/<pid>/<hex>`)
- `Id`: from `AutomationId` (only emitted if non-empty)
- `Bounds`: from `BoundingRectangle`
- `ActivationPoint`: from `GetClickablePoint()`, fallback to midpoint of bounds
- Native UIA properties: exposed in `native:` namespace via `GetPropertyProgrammaticName()` scan + `GetCurrentPropertyValueEx()`. Sentinels filtered.

**Type Conversion**: `VT_BOOL` → Bool, `VT_I*/VT_UI*` → Integer, `VT_R*/VT_DECIMAL/VT_DATE` → Number, `BSTR` → String, `SAFEARRAY(1D)` → Array.

**Patterns**:
- `Focusable`: `SetFocus()`
- `WindowSurface`: via `WindowPattern` + `TransformPattern` (activate, minimize, maximize, restore, move, resize, close)
- `accepts_user_input()`: heuristic `IsEnabled && IsInView` + `WaitForInputIdle` (100ms timeout)
- Virtualized elements: best-effort `VirtualizedItemPattern::Realize()` before child traversal

**Application Nodes**:
- Synthetic `app:Application` nodes group top-level elements by `CurrentProcessId`.
- RuntimeId: `uia://app/<pid>`
- Attributes: `ProcessId`, `Name` (filename without .exe), `ProcessName` (`app:` namespace, executable stem), `ExecutablePath`, `CommandLine`, `UserName`, `StartTime` (ISO-8601), `Architecture`.

**Root Streaming**: First `control:` desktop children (own process filtered), then one `app:Application` per seen PID in stable order.

**Error Handling**: Typed `UiaError` (thiserror) internally, mapped to `ProviderError` at boundaries.

**Shutdown**: `AtomicBool` guard prevents double shutdown; COM cleanup.

## 2a. Java Access Bridge provider (`provider-jab`, Swing/AWT)

Java Swing/AWT apps implement no UIA provider, so their windows are empty shells to the UIA provider. `crates/provider-jab` (`platynui-provider-jab`, `cfg(windows)`, descriptor id `jab`, technology `JAB`, `event_capabilities: None`) reads them through the JDK's own out-of-process channel — the Java Access Bridge — the same one screen readers use. **Nothing is loaded into the target JVM** beyond the JDK's own bridge, and the provider never mutates target-side configuration (no `.accessibility.properties`, no registry writes; pinned by the `no_configuration_mutation_code_paths_exist` unit test). It is registered via `platynui_link_os_providers!` alongside the UIA provider and structured after `provider-atspi`. Full design: OpenSpec change `add-jab-provider`.

**Threading model** — one dedicated pump thread owns *everything* JAB (`pump.rs`):
- Loads the client DLL, binds the lowercase-cdecl exports (`dll.rs`), calls `Windows_run()` (which creates the hidden rendezvous window **on the calling thread**), and runs the Win32 message pump that JVM discovery and callbacks require.
- Services all API calls from the typed `JabClient` (`client.rs`) over an mpsc channel, each with a per-call deadline (`providers.jab.call_timeout_ms`, default 2000 ms). No JAB function is ever called from another thread.
- Every bridge call is synchronous blocking IPC (`SendMessage` + shared memory) into the target JVM: a hung JVM blocks the pump inside the OS call, but callers time out promptly. After N consecutive timeouts a `vmID` is marked **degraded** and calls fail fast until a `getVersionInfo` health probe succeeds (`DegradedTracker`).
- When running elevated, `ChangeWindowMessageFilter` opens the UIPI filter for the bridge rendezvous messages (NVDA's workaround). An elevated *target* app remains out of reach.

**Handle discipline** — every `JOBJECT64` from the bridge is owned by a `JabObject` RAII wrapper (`handle.rs`); `Drop` enqueues a `releaseJavaObject` to the pump (Drop can run on any thread), which drains the queue between requests. Releases for a degraded JVM are deferred so the release itself cannot wedge the pump. Identity uses `isSameObject`, never raw handle equality (raw handles for the same object routinely differ). `is_valid()` is a cheap `isSameObject(ctx, ctx)` liveness probe.

**DLL discovery** (`dll.rs`, first hit wins): `providers.jab.dll_path` → `PLATYNUI_JAB_DLL` → `%JAVA_HOME%\jre\bin` → `%JAVA_HOME%\bin` → every `PATH` entry (the DLL directly, plus the JDK 8 quirk that `PATH` holds `<jdk>\bin` while `WindowsAccessBridge-64.dll` sits in `<jdk>\jre\bin`). Connection is lazy on first tree access; a missing DLL logs one actionable diagnostic and yields an empty child stream — runtime construction never fails because of JAB.

**Nodes & attributes** (`node.rs`) — context info is read **live** per attribute/children access (one `getAccessibleContextInfo` snapshot per logical `attributes()`/`children()` call, not cached across calls), so a state change shows on the next read. This matches UIA (live COM reads) and AT-SPI; a sticky node cache would go stale because the runtime reuses the XDM tree across queries without calling `invalidate()` on reused provider nodes.
- `Role`: PascalCase from `role_en_US` (`map.rs`), aligned to the AT-SPI2 vocabulary where they coincide; unknown roles PascalCased generically. Swing `spinbox` → `SpinButton`; a `label` child of a `list` carrying `selectable` is promoted to `item:ListItem`. Originals under `native:Role`/`native:LocalizedRole`.
- `Name`: accessible name. `Id`: never emitted (JAB has no AutomationId equivalent).
- `Bounds`: for **top-level windows** from the injected `WindowManager` (live `GetWindowRect`) — JAB frame bounds lag out-of-band moves; for **descendants** from JAB, through a self-calibrating per-window DPI transform (JAB is system-DPI-aware, PlatynUI is Per-Monitor-V2; the transform is derived from the window's JAB rect vs. `GetWindowRect`, identity at 100 %). The hidden-element sentinel `(-1,-1,-1×-1)` maps to "no Bounds".
- `IsEnabled`/`IsVisible`/`IsInView`/`IsFocused` and pattern states parsed from `states_en_US`. Pattern attributes: `ToggleState`, `Value`/`MinValue`/`MaxValue`, `IsSelected`, `SelectedItems`/`CanSelectMultiple`, `IsExpanded`/`CanExpand`, `Text` (chunked `getAccessibleTextRange`). `native:States`/`native:Interfaces`/`native:IndexInParent`/`native:NativeWindowHandle` passthrough.
- `RuntimeId`: `jab://<vmID>/0x<hwnd>[/<enum-index-path>]`, app view scoped `jab://app/<pid>/…` (mirrors UIA). The index path uses the *enumeration* index, not JAB's unreliable `indexInParent`. `app:Application` node is `jab://app/<pid>` with process metadata (sysinfo + PE-header architecture).

**Patterns**: Focusable (`requestFocus`), ActivationTarget (bounds center), TextContent + TextEditable (new core `TextEditableAction` → `setTextContents`, 1023-UTF-16-unit write limit), Toggleable, StatefulValue, Selectable/SelectionProvider, Expandable — each advertised only when the backing JAB interface/state is present. Window capability patterns on top-level nodes delegate to the injected `WindowManager` via `native:NativeWindowHandle` (the atspi blueprint), so activate/move/close/… reuse the Win32 implementation below.

**Single appearance**: the JAB provider registers each claimed Java HWND in a process-wide claims registry (`platynui_core::platform::window_claims`); the UIA provider skips windows claimed by another provider during root streaming (`providers.windows-uia.honor_window_claims`, default true). Kill switch off → both representations appear, distinguishable via `@Technology`.

**Enablement diagnostics**: a top-level whose class starts with `SunAwt` but which fails `isJavaWindow` triggers a warn-once-per-HWND diagnostic naming both enablement paths (the `-Djavax.accessibility.assistive_technologies=…AccessBridge` launch flag and `jabswitch -enable`). Never mutation.

**Events**: not registered in this MVP (`event_capabilities: None`, runtime polls — the same as UIA). JAB callbacks (`setPropertyStateChangeFP` etc.) are a genuine future enhancement for push-driven `event_capabilities` (targeted invalidation instead of polling); they are **not** needed for correct reads (see the live-read model above) and note that they must be registered *before* `Windows_run()` to fire.

**Config keys**: `providers.jab.enabled` (default true), `providers.jab.dll_path`, `providers.jab.call_timeout_ms` (default 2000); `providers.windows-uia.honor_window_claims` (default true).

## 3. WindowManager (Win32)

- `resolve_window()`: reads `native:NativeWindowHandle` → HWND (+ PID-fallback via `EnumWindows`)
- `bounds()`: `GetWindowRect(hwnd)` → desktop coordinates
- `is_active()`: `GetForegroundWindow() == hwnd`
- `activate()`: `ShowWindow(SW_RESTORE)` if minimized + `AttachThreadInput` bypass for foreground lock, then `BringWindowToTop(hwnd)` followed by `SetForegroundWindow(hwnd)`
- `close()`: `PostMessageW(WM_CLOSE)`
- `minimize/maximize/restore()`: `ShowWindow(SW_MINIMIZE/SW_MAXIMIZE/SW_RESTORE)`
- `move_to/resize()`: `SetWindowPos(hwnd, ...)`
