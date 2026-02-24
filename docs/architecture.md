# PlatynUI Architecture

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document describes the PlatynUI runtime architecture as it is currently implemented. It serves as the authoritative reference for the system's design, conventions, and platform-specific implementations.

## 1. Introduction & Goals

PlatynUI is a cross-platform UI automation library whose core provides a consistent view of native UI trees (UIA, AT-SPI2, macOS AX, ...). The runtime abstracts platform APIs into a normalized node tree that is queried via XPath and exposes capabilities through Patterns.

### Design Philosophy

- **Composition over inheritance** — Patterns describe capabilities as composable traits, not a class hierarchy.
- **Streaming & lazy evaluation** — Children, attributes, and pattern probes are evaluated on demand. No upfront materialization.
- **Platform-agnostic core** — `platynui-core` defines all traits; platform and provider crates implement them independently.
- **Build-time platform selection** — `cfg(target_os)` attributes determine which platform/provider crates are linked. No runtime platform selection (but see §3 for the planned Linux mediation crate).
- **Desktop-coordinate system** — All positions (`Bounds`, `ActivationPoint`, window geometry) use absolute desktop coordinates. DPI/scaling adjustments happen provider-side.

## 2. Crate Landscape

```
crates/
├─ core                      # Common traits/types — platynui-core
├─ xpath                     # XPath evaluator and parser — platynui-xpath
├─ runtime                   # Runtime, provider registry, XPath pipeline — platynui-runtime
├─ link                      # Linking helper macros — platynui-link
├─ platform-windows          # Windows devices — platynui-platform-windows
├─ provider-windows-uia      # UIA provider — platynui-provider-windows-uia
├─ platform-linux-x11        # Linux/X11 devices — platynui-platform-linux-x11
├─ provider-atspi            # AT-SPI2 provider — platynui-provider-atspi
├─ platform-macos            # macOS devices (stub) — platynui-platform-macos
├─ provider-macos-ax         # macOS AX provider (stub) — platynui-provider-macos-ax
├─ platform-mock             # Mock devices for testing — platynui-platform-mock
├─ provider-mock             # Mock UI tree provider — platynui-provider-mock
├─ cli                       # CLI tool — platynui-cli
└─ playground                # Development sandbox — playground

apps/
└─ inspector                 # GUI inspector (egui) — platynui-inspector

packages/
├─ native                    # Python bindings (PyO3/maturin) — platynui_native
├─ cli                       # Python wheel for CLI binary
└─ inspector                 # Python wheel for Inspector binary
```

### Naming Conventions

- Workspace crates (under `crates/`, `apps/`) use the `platynui-` package name prefix. Directory names may be shorter (e.g., `crates/runtime` → package `platynui-runtime`).
- FFI/packaging crates outside the Cargo workspace (e.g., `packages/native`) follow target ecosystem conventions and are exempt from the prefix rule.
- Platform crates: `crates/platform-<target>` (package `platynui-platform-<target>`)
- Provider crates: `crates/provider-<technology>` (package `platynui-provider-<technology>`)

### Dependency Graph

```
┌────────────────────────┐
│  Robot Framework Lib   │
│    (src/PlatynUI)      │
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐  ┌──────────┐  ┌─────────────┐
│  Python Bindings       │  │   CLI    │  │  Inspector  │
│  (packages/native,     │  │(crates/  │  │   (apps/    │
│   PyO3/maturin)        │  │  cli)    │  │  inspector) │
└───────────┬────────────┘  └────┬─────┘  └──────┬──────┘
            │                    │               │
            └────────────┬───────┘───────────────┘
                         │
                         ▼
                ┌───────────┐           ┌───────────────┐
                │  Runtime  │◄──────────│  Core traits  │
                │           │           │  & UI model   │
                └─────┬─────┘           └──────┬────────┘
                      │                        │
                ┌─────┴──────────┬─────────────┼──────┐
                │                │             │      │
                ▼                ▼             ▼      ▼
          ┌───────────┐  ┌─────────────┐  ┌──────────────┐
          │ Providers │  │  Platforms  │  │ XPath Engine │
          │ (UiTree)  │  │  (Devices)  │  │              │
          └───────────┘  └─────────────┘  └──────────────┘
```

Python wheels (`packages/cli`, `packages/inspector`) are packaging wrappers that bundle the Rust binaries for `pip install` distribution — they do not add Rust dependencies.

## 3. Registration & Extension Model

All extensions register via `inventory`-based macros. The runtime discovers them at link time without knowing concrete types.

### Registration Macros

- `register_platform_module!(&MODULE)` — registers a `PlatformModule` implementation
- `register_provider!(&FACTORY)` — registers a `UiTreeProviderFactory`
- `register_window_manager!(&PROVIDER)` — registers a `WindowManager` implementation

### Linking Strategy

- OS-specific providers register automatically when linked.
- Mock providers (`platynui-provider-mock`, `platynui-platform-mock`) do **not** auto-register and are only available via explicit factory handles.
- The helper crate `platynui-link` provides macros:
  - `platynui_link_providers!()` — feature-gated: links mock or OS providers
  - `platynui_link_os_providers!()` — explicitly links OS-specific crates
- Applications (CLI, Python extension) bind platform/provider crates via `cfg(target_os = ...)`.
- Tests link mock crates explicitly: `const _: () = { use platynui_platform_mock as _; use platynui_provider_mock as _; };`

### Planned: Linux Mediation Crate

Currently, Linux uses `platynui-platform-linux-x11` directly. For future Wayland support, the plan is a mediation crate `platynui-platform-linux` that bundles both `platform-linux-x11` and `platform-linux-wayland` and performs **runtime** session detection via `$XDG_SESSION_TYPE`:

- `x11` → delegate to X11 platform devices
- `wayland` → delegate to Wayland platform devices
- Fallback: attempt X11 first (XWayland compatibility)

This is the one exception to the "build-time platform selection" rule: on Linux, the display server is a runtime property. The mediation crate is registered via the same `inventory`-based mechanism.

### Provider Modes

- **In-process** — Rust crate linked directly (UIA, AT-SPI2, Mock)
- **Out-of-process** — planned (see `docs/planning.md` §3.4)

## 4. Runtime Context & Lifecycle

### Initialization Sequence

1. `Runtime::new()` calls `initialize_platform_modules()` — invokes `PlatformModule::initialize()` for all registered modules (e.g., Windows sets Per-Monitor-V2 DPI awareness).
2. Provider factories are instantiated via `ProviderRegistry`.
3. Event listeners are wired up via `UiTreeProvider::subscribe_events(listener)`.

### Shutdown & Resource Cleanup

- `Runtime::shutdown()` is called automatically via `Drop`. It is idempotent.
- The runtime shuts down providers first (calling `UiTreeProvider::shutdown()`), then platform modules.
- Providers must release all resources in `shutdown()` (COM handles, D-Bus connections, overlay windows).

### Test Injection

- `Runtime::new_with_factories(factories)` — builds runtime from explicit factories (no inventory discovery).
- `Runtime::new_with_factories_and_platforms(factories, PlatformOverrides)` — additionally injects mock platform devices.
- Central test helper: `runtime_with_factories_and_mock_platform(&[&FACTORY, ...])` in `crates/runtime/src/test_support.rs`.
- `rstest` fixtures: `rt_runtime_platform()` (mock devices, no providers), `rt_runtime_stub()`, `rt_runtime_focus()`.

## 5. Data Model

### 5.1 UiNode, UiAttribute, UiPattern Traits

```rust
pub trait UiNode: Send + Sync {
    fn namespace(&self) -> Namespace;
    fn role(&self) -> &str;                // e.g., "Window", "Button", "ListItem"
    fn name(&self) -> String;
    fn runtime_id(&self) -> &RuntimeId;
    fn id(&self) -> Option<String>;        // optional developer-set stable identifier
    fn parent(&self) -> Option<Weak<dyn UiNode>>;
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_>;
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_>;
    fn attribute(&self, namespace: Namespace, name: &str) -> Option<Arc<dyn UiAttribute>>;
    fn supported_patterns(&self) -> Vec<PatternId>;
    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>>;
    fn invalidate(&self);
}

pub trait UiAttribute: Send + Sync {
    fn namespace(&self) -> Namespace;
    fn name(&self) -> String;        // PascalCase
    fn value(&self) -> UiValue;      // lazily evaluated
}

pub trait UiPattern: Any + Send + Sync {
    fn id(&self) -> PatternId;
    fn static_id() -> PatternId where Self: Sized;
    fn as_any(&self) -> &dyn Any;
}
```

- Children and attributes are returned as `Box<dyn Iterator<...> + Send + '_>`. Providers may use custom iterator types.
- The runtime never materializes lists upfront; `UiAttribute::value()` is called on demand.
- `UiNodeExt` provides navigation helpers: `parent_arc()`, `ancestors()`, `top_level_or_self()`, `ancestor_pattern::<T>()`.
- `PatternRegistry` stores patterns as `HashMap<PatternId, Arc<dyn UiPattern>>` with insertion-order tracking. `register_lazy` defers expensive platform probes until first access.

### 5.2 Namespaces

| Prefix | Scope | Description |
|--------|-------|-------------|
| `control` | Default | UI controls (Window, Button, TextBox, ...) |
| `item` | Container children | ListItem, TreeItem, TabItem, ... |
| `app` | Application/process | Application nodes |
| `native` | Technology-specific | Raw platform attributes |

Expressions without a prefix match only `control:` elements. Use `item:` or wildcards to widen scope.

### 5.3 Desktop Document Node

- Platform crates provide a `DesktopInfoProvider` (registered via `inventory`). The runtime uses the first registered provider to build the desktop document node (XPath root).
- If no provider is registered, the runtime creates a fallback desktop with generic values (bounds 1920x1080, empty monitor list, technology "Fallback").
- `DesktopInfo` includes: OS version, monitor list (id, name, bounds, is_primary, scale_factor), desktop bounds.
- Provider views:
  1. **Flat view** — Top-level controls hang directly under the desktop in `control:` namespace.
  2. **Grouped view** — Same controls also appear as children of `app:Application` nodes, enabling queries like `app:Application[@Name='Studio']//control:Window`.

### 5.4 RuntimeId Schema

Format: `prefix:value` where the prefix identifies the provider/technology.

| Provider | Scheme | Example |
|----------|--------|---------|
| UIA | `uia://desktop/<hex>` or `uia://app/<pid>/<hex>` | `uia://desktop/2A0B3C` |
| AT-SPI2 | AT-SPI D-Bus object path | `atspi:///org/a11y/...` |
| Mock | `mock:<id>` | `mock:window-1` |
| Desktop | `platynui:Desktop` (reserved) | `platynui:Desktop` |

Providers generate deterministic IDs stable for the element's lifetime. The `platynui` prefix is reserved for the desktop node.

### 5.5 Developer Id (`control:Id`)

- Optional, developer-set stable identifier independent of visible labels and language.
- Platform mapping: Windows → `AutomationId`, Linux/AT-SPI2 → `accessible_id`, macOS/AX → `AXIdentifier`.
- Only emitted as an attribute when non-empty.
- For persistent selectors, prefer `@control:Id='...'` when available.

### 5.6 UiValue & Attribute Normalization

- **UiValue** is typed: String, Bool, Integer, Float, Null, plus structured values (Rect, Point, Size, Array, Object).
- **Alias attributes** — For structured values, the runtime/XPath layer auto-generates derived attributes: `Bounds.X`, `Bounds.Width`, `ActivationPoint.Y`, etc. Providers must not generate these aliases.
- **PascalCase** — All attribute names use PascalCase. Native roles are normalized (e.g., `UIA_ButtonControlTypeId` → `Button`). Original roles are preserved as `native:Role`.
- **Consistent naming constants** — `platynui-core::ui::attribute_names::<pattern>::*` provides string constants for attribute names.

### 5.7 Event Capabilities & Invalidation

`ProviderDescriptor` includes an `event_capabilities` bitset:

| Capability | Behavior |
|------------|----------|
| `None` | Runtime polls/refreshes before every query |
| `ChangeHint` | Provider signals "something changed"; runtime invalidates parent and re-queries children |
| `Structure` | Structural events with parent/RuntimeId; runtime handles affected subtrees selectively |
| `StructureWithProperties` | Additionally includes property change events |

- `TreeInvalidated` is the fallback for drastic changes (e.g., provider restart) and triggers a full reload.
- Providers must implement `UiNode::invalidate()` to discard cached data (children, attributes, patterns) so the next access fetches fresh data from the native API.
- Event-capable providers only trigger targeted updates; providers without events use full refresh before each query.

## 6. Pattern System

### 6.1 Design Principles

- **ClientPatterns** define attribute contracts (read-only capabilities): e.g., `TextContent`, `Selectable`, `ActivationTarget`.
- **RuntimePatterns** add actions: `FocusablePattern::focus()`, `WindowSurfacePattern::activate()`, etc.
- `SupportedPatterns` must be consistent: a pattern appears only if all mandatory attributes are available and a pattern instance exists.
- Clients decide how to interact with delivered information (mouse/keyboard simulation, gestures). This preserves the same interaction possibilities a human has.

### 6.2 Runtime Pattern Traits

```rust
pub trait FocusablePattern: UiPattern {
    fn focus(&self) -> Result<(), PatternError>;
}

pub trait WindowSurfacePattern: UiPattern {
    fn activate(&self) -> Result<(), PatternError>;
    fn minimize(&self) -> Result<(), PatternError>;
    fn maximize(&self) -> Result<(), PatternError>;
    fn restore(&self) -> Result<(), PatternError>;
    fn close(&self) -> Result<(), PatternError>;
    fn move_to(&self, position: Point) -> Result<(), PatternError>;
    fn resize(&self, size: Size) -> Result<(), PatternError>;
}
```

### 6.3 Pattern Catalog

#### ClientPatterns (Attribute Contracts)

| Pattern | Required Attributes | Optional Attributes |
|---------|-------------------|-------------------|
| **Element** | Bounds, IsVisible, IsEnabled | IsOffscreen, Technology |
| **Desktop** | Bounds, OsVersion, Monitors | — |
| **TextContent** | Text | IsReadOnly |
| **TextEditable** | Text, IsReadOnly=false | MaxLength |
| **TextSelection** | SelectedText, SelectionStart, SelectionEnd | — |
| **Selectable** | IsSelected | — |
| **SelectionProvider** | SelectedItems, CanSelectMultiple | — |
| **Toggleable** | ToggleState | — |
| **StatefulValue** | Value | MinValue, MaxValue |
| **Activatable** | — | — |
| **ActivationTarget** | ActivationPoint | ActivationArea |
| **Focusable** | IsFocused | — |
| **Scrollable** | ScrollHorizontalPercent, ScrollVerticalPercent, ScrollHorizontalViewSize, ScrollVerticalViewSize | HorizontallyScrollable, VerticallyScrollable |
| **Expandable** | IsExpanded | — |
| **ItemContainer** | — | — |
| **WindowSurface** | — | IsMinimized, IsMaximized, IsTopmost, SupportsMove, SupportsResize |
| **DialogSurface** | — | DialogResult |
| **Application** | ProcessId | ProcessName, ExecutablePath, CommandLine |
| **Highlightable** | — | — |
| **Annotatable** | — | — |

#### Pattern Composition Examples

- **Button** = TextContent + Activatable + ActivationTarget
- **ListItem** = TextContent + Selectable + ActivationTarget
- **Window** = Focusable + WindowSurface + ActivationTarget
- **TextBox** = TextContent + TextEditable + Focusable + ActivationTarget

### 6.4 Platform Mapping Tables

#### Role Mapping (Selection)

| PlatynUI Role | UIA ControlType | AT-SPI2 Role | macOS AX Role |
|---------------|-----------------|--------------|---------------|
| Button | Button | PUSH_BUTTON | AXButton |
| CheckBox | CheckBox | CHECK_BOX | AXCheckBox |
| ComboBox | ComboBox | COMBO_BOX | AXComboBox |
| Edit / TextBox | Edit | ENTRY / TEXT | AXTextField / AXTextArea |
| List | List | LIST | AXList |
| ListItem | ListItem | LIST_ITEM | AXStaticText (child) |
| Menu | Menu | MENU | AXMenu |
| MenuItem | MenuItem | MENU_ITEM | AXMenuItem |
| Tab | Tab | PAGE_TAB_LIST | AXTabGroup |
| TabItem | TabItem | PAGE_TAB | AXRadioButton (tab) |
| Tree | Tree | TREE_TABLE / TREE | AXOutline |
| TreeItem | TreeItem | TABLE_ROW / TREE_ITEM | AXRow |
| Window | Window | FRAME / WINDOW | AXWindow |
| Dialog | Window (IsDialog) | DIALOG | AXSheet / AXDialog |

#### Pattern-per-Platform Attribute Mapping

**TextContent**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| Text | NameProperty → ValuePattern.Value → TextPattern.DocumentRange.GetText | Text.GetText(0, -1) | AXValue / AXTitle |
| IsReadOnly | ValuePattern.IsReadOnly | Text.NCharacters == 0 hint | AXEditable (inverted) |

**TextEditable** — extends TextContent with write access:

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| Text (write) | ValuePattern.SetValue / TextPattern ranges | EditableText.SetTextContents / InsertText | AXValue (settable) |
| MaxLength | — | — | — |

**TextSelection**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| SelectedText | TextPattern.GetSelection → GetText | Text.GetSelection → GetText | AXSelectedText |
| SelectionStart | TextPattern.GetSelection range start | Text.GetSelection nStartOffset | AXSelectedTextRange.location |
| SelectionEnd | TextPattern.GetSelection range end | Text.GetSelection nEndOffset | AXSelectedTextRange.location + length |

**Selectable / SelectionProvider**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| IsSelected | SelectionItemPattern.IsSelected | State.SELECTED | AXSelected |
| SelectedItems | SelectionPattern.GetSelection | Selection.GetSelectedChildren | AXSelectedChildren |
| CanSelectMultiple | SelectionPattern.CanSelectMultiple | Selection.NSelectedChildren context | AXAllowsMultipleSelection |

**Toggleable**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ToggleState | TogglePattern.ToggleState (On/Off/Indeterminate) | State.CHECKED / State.INDETERMINATE | AXValue (0/1/2) |

**StatefulValue**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| Value | RangeValuePattern.Value / ValuePattern.Value | Value.CurrentValue | AXValue |
| MinValue | RangeValuePattern.Minimum | Value.MinimumValue | AXMinValue |
| MaxValue | RangeValuePattern.Maximum | Value.MaximumValue | AXMaxValue |

**ActivationTarget**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ActivationPoint | GetClickablePoint() or Bounds center | Component.GetExtents(Screen) center | AXPosition center |
| ActivationArea | BoundingRectangle | Component.GetExtents(Screen) | AXFrame |

**Focusable**

| Attribute / Action | UIA | AT-SPI2 | macOS AX |
|--------------------|-----|---------|----------|
| IsFocused | HasKeyboardFocus | State.FOCUSED | AXFocused |
| focus() | SetFocus() | grab_focus() | AXFocused = true |

**Scrollable**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ScrollHorizontalPercent | ScrollPattern.HorizontalScrollPercent | — (compute from Value) | — |
| ScrollVerticalPercent | ScrollPattern.VerticalScrollPercent | — (compute from Value) | — |
| HorizontallyScrollable | ScrollPattern.HorizontallyScrollable | — | — |
| VerticallyScrollable | ScrollPattern.VerticallyScrollable | — | — |

**Expandable**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| IsExpanded | ExpandCollapsePattern.ExpandCollapseState | State.EXPANDED / State.EXPANDABLE | AXExpanded |

**WindowSurface**

| Attribute / Action | UIA | AT-SPI2 | macOS AX |
|--------------------|-----|---------|----------|
| activate() | WindowPattern.SetWindowVisualState(Normal) + SetFocus | EWMH _NET_ACTIVE_WINDOW | AXRaise + AXFocused |
| minimize() | WindowPattern.SetWindowVisualState(Minimized) | EWMH _NET_WM_STATE | AXMinimized = true |
| maximize() | WindowPattern.SetWindowVisualState(Maximized) | EWMH _NET_WM_STATE | AXFullScreen (approx) |
| close() | WindowPattern.Close() | EWMH _NET_CLOSE_WINDOW | AXClose action |
| move_to() | TransformPattern.Move(x, y) | EWMH _NET_MOVERESIZE_WINDOW | AXPosition = (x, y) |
| resize() | TransformPattern.Resize(w, h) | EWMH _NET_MOVERESIZE_WINDOW | AXSize = (w, h) |
| IsMinimized | WindowPattern.WindowVisualState == Minimized | State.ICONIFIED | AXMinimized |
| IsMaximized | WindowPattern.WindowVisualState == Maximized | EWMH _NET_WM_STATE check | AXFullScreen |
| IsTopmost | WindowPattern.IsTopmost | EWMH _NET_WM_STATE_ABOVE | AXMain hint |
| accepts_user_input() | IsEnabled && !IsOffscreen + WaitForInputIdle | State.SENSITIVE && State.SHOWING | AXEnabled |

**Application**

| Attribute | UIA | AT-SPI2 | macOS AX |
|-----------|-----|---------|----------|
| ProcessId | CurrentProcessId | D-Bus peer credentials | AXPid (via kAXPIDAttribute) |
| ProcessName | Executable filename (without .exe) | /proc/PID/cmdline or D-Bus | NSRunningApplication.localizedName |
| ExecutablePath | OpenProcess + QueryFullProcessImageName | /proc/PID/exe | NSRunningApplication.executableURL |

## 7. Provider Infrastructure

### 7.1 UiTreeProvider & Factory Traits

- `ProviderDescriptor` describes an implementation: `id`, display name, `TechnologyId`, `ProviderKind` (Native or External), `event_capabilities`.
- `UiTreeProviderFactory::create()` returns `Arc<dyn UiTreeProvider>`. No additional resources are passed.
- `UiTreeProvider::get_nodes(parent)` returns nodes for the given parent. The runtime combines them with the desktop node.

### 7.2 Provider Registry & Event Pipeline

- `ProviderRegistry` (in `crates/runtime`) collects registered factories via `inventory`, groups them by technology, and creates instances.
- `ProviderEventDispatcher` — fan-out component that synchronously relays provider events to registered sinks.
- Per provider, a `RuntimeEventListener` (a) marks the snapshot as dirty, (b) invalidates affected `UiNode` instances, (c) forwards the event to the dispatcher.
- `ProviderEventKind`: `NodeAdded`, `NodeUpdated`, `NodeRemoved`, `TreeInvalidated`.
- External consumers (CLI, Inspector) subscribe via `Runtime::register_event_sink`.

### 7.3 Provider Compliance Checklist

All providers must:

- Register via inventory macros; productive implementations use `cfg(target_os = ...)`.
- Provide complete `ProviderDescriptor` with accurate `event_capabilities`.
- Use lazy evaluation for `UiNode` and `UiAttribute` implementations.
- Use correct namespaces (`control`, `item`, `app`, `native`).
- Deliver all coordinates in the desktop coordinate system (DPI-aware).
- Maintain stable `RuntimeId` values for element lifetimes (format: `prefix:value`).
- Emit `Id` only when non-empty.
- Set `parent` references correctly in children iterators.
- Keep `SupportedPatterns` consistent with available pattern instances.
- Normalize roles to PascalCase; preserve originals under `native:*`.
- Filter out own-process windows/overlays from the UI tree.
- Implement `shutdown()` for resource cleanup.
- Implement `invalidate()` to discard cached data.
- Provided attributes match ClientPattern requirements from the pattern catalog (§6.3) — use constants from `platynui_core::ui::attribute_names`. The core testkit (`platynui_core::ui::contract::testkit`) validates this automatically.
- Keyboard device: key naming case-insensitive lookup, `known_key_names()` stable-sortable, `UnsupportedKey` errors informative.
- `ActivationTarget` provides `ActivationPoint` (native API preferred, bounds center as fallback).

#### Platform-Specific Checklist: Windows (UIA)

- Bounds from `BoundingRectangle` in desktop coordinates (Per-Monitor-V2 DPI active).
- ActivationPoint via `GetClickablePoint()` or center of bounds as fallback.
- Text priority: `NameProperty` → `ValuePattern.Value` → `TextPattern.DocumentRange.GetText`.
- WindowSurface attributes via `WindowPattern`/`TransformPattern`; `accepts_user_input()` via `IsEnabled && !IsOffscreen` + `WaitForInputIdle`.
- Application node: process metadata (`ProcessId`, `Name`, `ExecutablePath`, `CommandLine`, `UserName`, `StartTime`, `Architecture`).
- `SelectionItemPattern`/`SelectionPattern` sync verified.
- COM initialized (`CoInitializeEx` MTA) before any UIA call.
- `VirtualizedItemPattern::Realize()` attempted before child traversal.
- Native properties via `GetCurrentPropertyValueEx(id, true)` with sentinel filtering (`ReservedNotSupportedValue`, `ReservedMixedAttributeValue`).
- VARIANT type conversion: `VT_BOOL` → Bool, `VT_I*/VT_UI*` → Integer, `VT_R*/VT_DECIMAL` → Number, `BSTR` → String, `SAFEARRAY(1D)` → Array.

#### Platform-Specific Checklist: Linux (AT-SPI2 + X11)

- Coordinates from `Component.GetExtents(ATSPI_COORD_TYPE_SCREEN)`.
- TextContent/TextSelection via AT-SPI `Text` interface.
- SelectionProvider via AT-SPI `Selection` interface.
- WindowSurface actions delegate to EWMH WindowManager (not direct X11 calls from provider).
- Component-gated attributes (`Bounds`, `ActivationPoint`, `IsEnabled`, `IsVisible`, `IsOffscreen`, `IsFocused`) only present when Component interface available.
- Native attributes: `Native/<Interface>.<Property>` format for all AT-SPI interfaces.
- Virtual desktops: provider does NOT handle desktop switching — that is `WindowManager::ensure_window_accessible()` responsibility.

#### Platform-Specific Checklist: macOS (AX)

- Coordinates from `AXFrame` in Core Graphics desktop coordinate system.
- `TextEditable` via `AXEditable`/`AXEnabled`.
- `Activatable` via `AXPress` action.
- WindowSurface via accessibility actions + `CGWindow`/`NSWorkspace`.
- `kAXRaiseAction` for window activation (implicitly switches Spaces).

#### Platform-Specific Checklist: Mock

- Reference data covers typical pattern combinations (Button, Window, TextBox, List, TreeItem).
- Desktop coordinates tested (multi-monitor arrangement: 2160x3840 left, 3840x2160 center primary, 1920x1080 right).
- Scripted mock allows negative scenarios (missing patterns, failing focus).
- Pointer/keyboard mock produce deterministic logging (`take_pointer_log`, `take_keyboard_log`, `reset_*` helpers).
- Text buffer operations: `append_text`, `replace_text`, `apply_keyboard_events`.

## 8. Platform Devices

### 8.1 PointerDevice

The `PointerDevice` trait in `platynui-core` provides elementary pointer input in desktop coordinates (`f64`):

- `position() -> Point`, `move_to(Point)`, `press(PointerButton)`, `release(PointerButton)`, `scroll(ScrollDelta)`
- Optional: `double_click_time()`, `double_click_size()`

The runtime builds a motion engine on top:

- **Motion modes**: `direct`, `linear`, `bezier`, `overshoot`, `jitter`
- **Acceleration profiles**: constant, slow→fast, fast→slow, smooth S-curve
- **Configurable delays**: `after_move_delay`, `press_release_delay`, `before_next_click_delay`, `multi_click_delay`
- **Position verification**: optional check that cursor reached target
- **PointerProfile** bundles movement and timing parameters as named presets (e.g., `default`, `fast`, `human-like`).
- **PointerOverrides** provides per-call delta overrides via builder API. Includes optional `origin` field (`PointOrigin::Desktop`, `PointOrigin::Bounds(Rect)`, `PointOrigin::Absolute(Point)`) for relative coordinate conversion.

Runtime methods: `pointer_move_to`, `pointer_click`, `pointer_multi_click`, `pointer_press`, `pointer_release`, `pointer_drag`, `pointer_scroll`. All accept optional `PointerOverrides`.

### 8.2 KeyboardDevice

The `KeyboardDevice` trait provides:

- `key_to_code(&str) -> Result<KeyCode, KeyboardError>` — resolves key names/aliases to provider-specific key codes
- `send_key_event(KeyboardEvent) -> Result<(), KeyboardError>` — sends a single press or release
- `start_input()` / `end_input()` — optional hooks for keyboard-specific preparation (e.g., IME switching)
- `known_key_names() -> Vec<String>` — lists supported key names

**KeyboardEvent** is a struct with `KeyCode` and `KeyState` (Press/Release).

**KeyboardSequence** is the runtime's central input representation. It parses mixed inputs like `"text<Ctrl+a><Ctrl+Delete>Hello"`, backslash escapes (`\\<`, `\\>`, `\\`, `\\xNN`, `\\uNNNN`), and multi-shortcuts (`<Ctrl+K Ctrl+C>`) into a lazy sequence of key events. Unknown key names cause `KeyboardError::UnsupportedKey`.

**Symbol aliases** for reserved characters in shortcuts: `PLUS` (+), `MINUS` (-), `LESS`/`LT` (<), `GREATER`/`GT` (>).

**Key naming conventions**: Platform-official names without prefixes (e.g., Windows uses `ESCAPE`, not `VK_ESCAPE`). Common keys share names across platforms (`Enter`, `Escape`, `Shift`). Platform-specific keys use established OS terms (`Command`/`Option` on macOS, `Windows` on Windows, `Super`/`Meta` on Linux).

Runtime APIs:
- `keyboard_type(sequence, overrides)` — press→release for each step
- `keyboard_press(sequence, overrides)` — press-only
- `keyboard_release(sequence, overrides)` — release-only

**KeyboardSettings** holds global defaults (`press_delay`, `release_delay`, `between_keys_delay`, `chord_press_delay`, `chord_release_delay`, `after_sequence_delay`, `after_text_delay`). **KeyboardOverrides** provides per-call deltas.

### 8.3 ScreenshotProvider

- `ScreenshotRequest` optionally specifies a sub-region; otherwise the full desktop is captured.
- `Screenshot` contains width, height, raw data (`Vec<u8>`), and `PixelFormat` (`Rgba8` or `Bgra8`).
- Callers are responsible for encoding to PNG/JPEG.

### 8.4 HighlightProvider

- `highlight(&HighlightRequest)` draws highlights; `clear()` removes them.
- `HighlightRequest` contains one or more desktop bounding boxes (`Vec<Rect>`) and an optional duration (`Duration`).
- Only one active highlight at a time; new requests replace the existing overlay.
- The provider auto-clears after the requested duration (timer-based).

### 8.5 WindowManager

```rust
pub struct WindowId(u64);  // Opaque: HWND, XID, Wayland surface ID

pub trait WindowManager: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError>;
    fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError>;
    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError>;
    fn activate(&self, id: WindowId) -> Result<(), PlatformError>;
    fn close(&self, id: WindowId) -> Result<(), PlatformError>;
    fn minimize(&self, id: WindowId) -> Result<(), PlatformError>;
    fn maximize(&self, id: WindowId) -> Result<(), PlatformError>;
    fn restore(&self, id: WindowId) -> Result<(), PlatformError>;
    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError>;
    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError>;
}
```

**`resolve_window` accepts `&dyn UiNode`** — each platform extracts what it needs:
- **Windows**: reads `native:NativeWindowHandle` → HWND (+ PID-fallback via `EnumWindows`)
- **X11**: walks parent chain for `control:ProcessId`, matches via `_NET_CLIENT_LIST` + `_NET_WM_PID`; disambiguates multi-window PIDs by comparing `node.name()` against `_NET_WM_NAME`

**Virtual Desktop Switching** (planned, not yet implemented) — `ensure_window_accessible()` will be added to the `WindowManager` trait and called in `bring_to_front()` before `activate()`. See `docs/planning.md` §3.2 for the design:
- **X11**: read `_NET_WM_DESKTOP`, switch via `_NET_CURRENT_DESKTOP` ClientMessage if different
- **Windows**: `IVirtualDesktopManager::MoveWindowToDesktop` to move window to current desktop
- **macOS**: no-op (`kAXRaiseAction` implicitly switches spaces)

**Layer model**:
```
provider-atspi / provider-windows-uia  ──uses──►  core::WindowManager (trait)
                                                            ▲          ▲
                                            platform-linux-x11   platform-windows
                                              (EWMH/NetWM)       (Win32 HWND)
                                        (future: platform-linux-wayland)
```

This keeps `provider-atspi` free of `x11rb` dependencies and working identically on X11 and future Wayland.

**Wayland Protocol Landscape** — Wayland has no standardized protocol for desktop/workspace switching by external clients. The following protocols exist:

| Protocol / API | Compositor | Capabilities |
|---|---|---|
| `wlr-foreign-toplevel-management-unstable-v1` | wlroots (Sway, Hyprland) | Activate, minimize, maximize, close windows. **No desktop switch.** |
| `ext-foreign-toplevel-list-v1` | Draft standard | Read-only listing of toplevel windows only |
| `cosmic-toplevel-info-unstable-v1` + `cosmic-workspace-unstable-v1` | COSMIC/Pop!_OS | Workspace listing and activation |
| KDE D-Bus (`org.kde.KWin`) | KDE-specific | `setCurrentDesktop`, `windowToDesktop` |
| GNOME Shell D-Bus (`org.gnome.Shell`) | GNOME-specific | Extensions API, not official |

Currently irrelevant since PlatynUI uses X11/XWayland. Long-term, Wayland support would require either compositor-specific backends or waiting for a cross-compositor protocol. See §4.4 in planning.md for details.

## 9. XPath Engine

### 9.1 Architecture

The XPath engine (`crates/xpath`, package `platynui-xpath`) implements XPath 2.0 evaluation in four layers:

1. **Parser** — PEG grammar (pest) produces a strongly-typed `Expr` AST
2. **Compiler** — Transforms AST into optimized IR (pre-calculated literals, merged filter predicates, specialized axis steps)
3. **Engine** — Evaluates IR against the XDM tree with `DynamicContext` and `StaticContext`
4. **Model** — `XdmNode` trait enables lazy tree traversal

```rust
pub trait XdmNode: Clone + Debug + Send + Sync + 'static {
    fn node_kind(&self) -> NodeKind;
    fn name(&self) -> Option<QName>;
    fn string_value(&self) -> Cow<'_, str>;
    fn typed_value(&self) -> Vec<XdmAtomicValue>;
    fn children(&self) -> Box<dyn Iterator<Item = Self> + '_>;  // lazy
    fn attributes(&self) -> Box<dyn Iterator<Item = (QName, String)> + '_>;
    fn parent(&self) -> Option<Self>;
    // ...
}
```

### 9.2 Streaming & Normalization

The engine operates in streaming mode — partial results are yielded immediately and predicates are evaluated early. XPath 2.0-mandated normalization (document order + deduplication) is split into two explicit IR operations:

- **`EnsureDistinct`** — removes duplicates while preserving order; implemented as a cursor (fully streaming).
- **`EnsureOrder`** — enforces document order. Passes monotone input through directly, repairs simple inversions locally, falls back to buffering+sorting only for true disorder.

**Emission rules** (conservative, spec-compliant):
- Forward axes `child`, `self`, `attribute`, `namespace`: no normalization
- Forward axes `descendant`, `descendant-or-self`, `following`, `following-sibling`: `EnsureDistinct`
- Reverse axes `parent`, `ancestor*`, `preceding*`: `EnsureDistinct` + `EnsureOrder`
- Path steps and set operations are normalized before the next step

Context minimization before certain axes (`descendant*`, `following*`) removes overlapping contexts at the source, often eliminating the need for post-hoc normalization.

### 9.3 XDM Cache

- `XdmCache` is `Rc<RefCell<Option<(RuntimeId, RuntimeXdmNode)>>>` — `Clone`, `!Send`. Created by the caller via `Runtime::create_cache()`, not held by the runtime.
- **Lazy Revalidation**: `is_valid()` checks provider nodes; `prepare_for_evaluation()` resets `children_validated` flags; invalid subtrees are transparently rebuilt on next access.
- Convenience methods: `evaluate_cached()`, `evaluate_iter_cached()`, `evaluate_single_cached()`.
- Without cache, the runtime rebuilds the desktop snapshot before every XPath evaluation.

### 9.4 Evaluation API

- `evaluate(node: Option<Arc<dyn UiNode>>, xpath: &str, options)` — central interface. Without context (`None`), evaluation starts at the desktop.
- `EvaluateOptions` supports `with_cache()` / `without_cache()` and `with_node_resolver(...)` for re-resolving context nodes by `RuntimeId`.
- `evaluate_iter_owned(...)` returns an owned iterator (`EvaluationStream`) for FFI bindings (no borrowed references).
- Results are `EvaluationItem`: `Node`, `Attribute` (owner + name + value), or `Value` (`UiValue`).
- `StaticContext` registers fixed prefixes: `control`, `item`, `app`, `native`.
- `typed_value()` is mandatory and returns XDM-conformant atomics (`xs:boolean`, `xs:integer`, `xs:double`, `xs:string`). Complex structures (Rect, Point, Size) remain as JSON-encoded strings; their derived components (`Bounds.X`, etc.) are numeric atomics.

## 10. Platform Implementations

Detailed platform-specific documentation has been extracted into separate files:

- **Windows** (UIA, Win32 devices, WindowManager): [`docs/platform-windows.md`](platform-windows.md)
- **Linux** (X11 devices, AT-SPI2, EWMH WindowManager): [`docs/platform-linux.md`](platform-linux.md)

## 11. Companion Documentation

- **Python Bindings** (PyO3, type mapping, threading): [`docs/python-bindings.md`](python-bindings.md)
- **CLI** (commands, snapshot model): [`docs/cli.md`](cli.md)
- **Inspector** (TreeView architecture): [`docs/inspector.md`](inspector.md)
- **Logging & Tracing**: [`.github/instructions/tracing.instructions.md`](../.github/instructions/tracing.instructions.md)
