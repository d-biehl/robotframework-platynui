# Windows Platform Implementation

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the Windows-specific implementation details for PlatynUI: platform devices, UIA provider, and Win32 WindowManager. For the platform-agnostic architecture, see `docs/architecture.md`.

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
- No `FindAll`, no UIA `CacheRequest`.
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
- `accepts_user_input()`: heuristic `IsEnabled && !IsOffscreen` + `WaitForInputIdle` (100ms timeout)
- Virtualized elements: best-effort `VirtualizedItemPattern::Realize()` before child traversal

**Application Nodes**:
- Synthetic `app:Application` nodes group top-level elements by `CurrentProcessId`.
- RuntimeId: `uia-app://<pid>`
- Attributes: `ProcessId`, `Name` (filename without .exe), `ExecutablePath`, `CommandLine`, `UserName`, `StartTime` (ISO-8601), `Architecture`.

**Root Streaming**: First `control:` desktop children (own process filtered), then one `app:Application` per seen PID in stable order.

**Error Handling**: Typed `UiaError` (thiserror) internally, mapped to `ProviderError` at boundaries.

**Shutdown**: `AtomicBool` guard prevents double shutdown; COM cleanup.

## 3. WindowManager (Win32)

- `resolve_window()`: reads `native:NativeWindowHandle` → HWND (+ PID-fallback via `EnumWindows`)
- `bounds()`: `GetWindowRect(hwnd)` → desktop coordinates
- `is_active()`: `GetForegroundWindow() == hwnd`
- `activate()`: `SetForegroundWindow(hwnd)` + `ShowWindow(SW_RESTORE)` if minimized + `AttachThreadInput` bypass for foreground lock
- `close()`: `PostMessageW(WM_CLOSE)`
- `minimize/maximize/restore()`: `ShowWindow(SW_MINIMIZE/SW_MAXIMIZE/SW_RESTORE)`
- `move_to/resize()`: `SetWindowPos(hwnd, ...)`
