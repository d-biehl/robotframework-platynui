# Testing Strategy

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

> **Note:** This document is a **collection of ideas and a basis for discussion**. It describes the current state of testing, identified gaps, and possible approaches for a systematic testing strategy. Nothing here is final — the document serves as a foundation for further planning and prioritization.

## 1. Current State

### 1.1 Test Pyramid (as-is)

```
                    ┌──────────────┐
                    │ Manual Robot │  ← 4 .robot files, live OS apps
                    │ Experiments  │    No CI, no assertions
                    └──────────────┘
                                       (Gap: no RF integration tests)
               ┌────────────────────────┐
               │  Python Binding Tests  │  ← ~42 tests, mock only
               └────────────────────────┘
          ┌──────────────────────────────────┐
          │  Rust Unit + Integration Tests   │  ← ~1,847 tests
          │  (XPath: ~1,542, Runtime/CLI/..) │    predominantly mock-based
          └──────────────────────────────────┘
```

### 1.2 Strengths

- **XPath Engine** is thoroughly tested (~1,542 tests): parser, compiler, evaluator, functions, streaming, optimizer, types, casts, property-based tests.
- **Mock Infrastructure** is comprehensive: Mock Platform + Mock Provider + Mock Tree with introspection logging for deterministic tests without native APIs.
- **Contract Verification**: The `testkit` module in `platynui_core::ui::contract` provides structured pattern/attribute validation.
- **Python Bindings** well covered: ~42 tests for runtime, geometry types, iterators, overrides, screenshots, focus, mock tree.
- **CLI** thoroughly tested: ~58 tests for all commands with mock backends.

### 1.3 Gaps

| Area | Problem |
|---|---|
| **Rust Platform Tests** | `provider-windows-uia` has 0 tests, `provider-macos-ax` has 0 tests, `platform-windows` only 9, `platform-linux-x11` only 9 keyboard tests. No live platform tests in CI. |
| **Python BareMetal Keywords** | 0 tests. The 21 keywords (`Query`, `Focus`, `Pointer Click`, `Keyboard Type` etc.) are completely untested at the Python level. |
| **Robot Framework Integration** | No deterministic RF tests. The 4 `.robot` files in `tests/BareMetal/` are manual demos against live apps (Notepad, calc.exe, kalk). |
| **Cross-Platform CI** | CI only runs on Linux. Windows/macOS are used only for wheel builds, not for testing. |
| **Test Application** | No dedicated test app. All RF tests depend on pre-installed OS apps (Notepad, Calculator, KDE Kalk). |
| **Provider Contract Tests** | Shared contract test suite exists in core but is only used by the mock provider, not by the real providers. |

### 1.4 Existing Robot Framework Tests

The `.robot` files under `tests/BareMetal/` are currently more of a playground for experimentation:

| File | Description | Platform |
|---|---|---|
| `anv.robot` | Keyboard typing in Notepad, pointer movement | Windows |
| `calc.robot` | Calculator automation: enter digits, find buttons | Windows |
| `demo.robot` | KDE Kalk calculator via pointer clicks | Linux |
| `simple.robot` | Pointer Move/Click, screenshots, highlight, focus | General |

These tests have no real assertions, are not CI-capable, and require running desktop applications.

### 1.5 Mock Infrastructure — What Can Be Tested Right Now?

The existing mock infrastructure (`platynui-platform-mock` + `platynui-provider-mock`) provides a complete simulation without native APIs. Both crates deliver deterministic behavior with introspection logging (`take_pointer_log()`, `take_keyboard_log()`, `take_highlight_log()` etc.) for verification.

#### Prerequisite: Cargo Feature `mock-provider`

The mock provider is **not a default feature** — it must be explicitly enabled at build time. The feature chain works as follows:

```
platynui_native/mock-provider
  → platynui-runtime/mock-provider
    → enables: platynui-provider-mock + platynui-platform-mock
```

| Context | Mock available? | Explanation |
|---|---|---|
| **Rust `#[cfg(test)]` / `[dev-dependencies]`** | **Always** | Mock crates are available as unconditional `dev-dependencies`. No feature flag needed. |
| **Python pytest / Robot Framework** | **Only with feature** | The native package must be built with `--features mock-provider`, otherwise `Runtime.new_with_mock()` raises a `ProviderError`. |
| **CLI (`platynui query --mock`)** | **Only with feature** | The CLI must be compiled with `--features mock-provider`. |

**Build Commands for Mock Tests:**

```bash
# Native Python package with mock support (for pytest + RF)
uv run maturin develop -m packages/native/Cargo.toml --features mock-provider

# CLI with mock support
uv run maturin develop -m packages/cli/Cargo.toml --features mock-provider

# Rust tests don't need the feature (dev-dependencies suffice)
cargo nextest run --all --no-fail-fast
```

**Usage in Tests:**

```python
# pytest (conftest.py)
@pytest.fixture
def rt_mock_platform():
    runtime = Runtime.new_with_mock()   # requires mock-provider feature
    yield runtime
    runtime.shutdown()
```

```robotframework
# Robot Framework
Library    PlatynUI.BareMetal    use_mock=${true}
# BareMetal internally calls Runtime.new_with_mock()
```

> **Important:** Without `--features mock-provider`, all mock-based Python and RF tests fail with: `ProviderError: Runtime.new_with_mock() requires building with feature 'mock-provider'`.

#### Mock Tree (`crates/provider-mock/assets/mock_tree.xml`)

```
app:Application "Mock Application"
├── Window "Operations Console" (Focusable, WindowSurface, Bounds, AutomationId)
│   └── Panel "Workspace"
│       ├── List "Task List" → 4 ListItems
│       ├── Tree "Navigation" → 7 TreeItems (2 levels deep)
│       ├── Text "Status" (text: "Ready")
│       ├── Button "OK" (IsFocused=true, ActivationPoint, MyProperty)
│       └── Button "Cancel" (ActivationPoint)
└── Window "Detail View"
    └── Text "Description"

app:Application "Mock Settings"
└── Window "Settings" (AXSubrole)
    ├── CheckBox "Send reports automatically"
    └── ComboBox "Theme" → 3 ComboBoxItems (Light, Dark, System)
```

#### Testable Keywords (Mock)

| Keyword | Mock Capability | Verification |
|---|---|---|
| `Query` | Full — XPath against mock tree (Buttons, Lists, Trees, Windows, Apps) | Return value: UiNode(s) with Name, Bounds etc. |
| `Set Root` | Full — Scoped queries on Window/Panel | Return value, follow-up queries |
| `Get Attribute` | Full — Bounds, Name, IsFocused, ActivationPoint, IsVisible, IsEnabled, Technology | Attribute values deterministically comparable |
| `Focus` | Full — OK button has Focusable pattern, IsFocused changes | IsFocused attribute after Focus == true |
| `Pointer Click` | Full — Mock pointer logs target coordinates and button | `take_pointer_log()` in pytest; in RF: no error + return |
| `Pointer Multi Click` | Full — Double/triple click | Mock log |
| `Pointer Press` / `Release` | Full | Mock log |
| `Pointer Move To` | Full — To element (ActivationPoint) or coordinates | Mock log, position |
| `Get Pointer Position` | Full — Deterministic position | Return value |
| `Keyboard Type` | Full — Text, special characters, shortcuts | `take_keyboard_log()` in pytest; in RF: no error |
| `Keyboard Press` / `Release` | Full | Mock log |
| `Take Screenshot` | Full — Deterministic RGBA gradient image | Return: bytes > 0 |
| `Highlight` | Full — Rect or element | Mock log |
| `Activate` / `Minimize` / `Maximize` / `Restore` / `Close` | Full — Mock windows have WindowSurface pattern | `take_window_manager_log()`, no error |
| `Bring To Front` | Full — Mock WindowManager | Mock log |

#### Testable RF Scenarios (Mock)

| Scenario | Description |
|---|---|
| Element search | `Query //Button[@Name='OK']` → Node found, Name/Bounds correct |
| Scoped queries | `Set Root` on Window → `Query .//Button` finds only buttons in that window |
| Attribute verification | `Get Attribute` for Bounds, Name, IsFocused → expected values |
| Focus workflow | Query → Focus → Get Attribute IsFocused → `${true}` |
| Window operations | Query Window → Activate → Minimize → Restore → Close |
| Screenshot smoke | Take Screenshot → File exists, size > 0 |
| Error handling | Query invalid XPath → error; Focus on non-focusable element → error |
| Multiple results | Query `//ListItem` → 4 items, correct names |
| Nested search | Query TreeItems (Dashboard/Overview/Metrics, Reports/Daily/Monthly/Yearly) |
| Cross-namespace | Query `app:Application[@Name='Mock Application']` → Application node |
| Pointer workflow | Query Element → Pointer Click → no error (mock logs action) |
| Keyboard workflow | Keyboard Type "Hello" → no error |

#### Not Testable via Mock — Extension Plan

The mock provider is architecturally designed for extensibility: XML parsing, `PatternRegistry`, lazy registration, and the event system (`emit_node_updated`) are all in place. New patterns follow the schema of `focus.rs` (~114 lines) or `window.rs` (~351 lines).

**Current State:** Only **2 of ~14 planned patterns** have runtime actions (`Focusable`, `WindowSurface`). 4 more (`Application`, `Element`, `TextContent`, `ActivationTarget`) provide attributes without action traits. The remaining pattern attributes (58 constants in `core::ui::attributes`) are not used by the mock.

**Planned Mock Extensions:**

| Pattern | New Elements in Mock Tree | New Attributes | New Actions | Core Change | Effort |
|---|---|---|---|---|---|
| **Toggleable** | CheckBox gets pattern | `ToggleState`, `SupportsThreeState` | `toggle()` changes state (On→Off→On) | New trait `ToggleablePattern` in `pattern.rs` | ~1–3h |
| **Selectable** | ListItems + ComboBoxItems | `IsSelected`, `SelectionOrder` | `select()`, `deselect()` | New trait `SelectablePattern` | ~1–3h |
| **SelectionProvider** | List + ComboBox | `SelectionMode`, `SelectedIds` | `select_by_id()`, `clear_selection()` | New trait `SelectionProviderPattern` | ~2–4h |
| **Expandable** | TreeItems (Dashboard, Reports) | `IsExpanded`, `HasChildren` | `expand()`, `collapse()` | New trait `ExpandablePattern` | ~1–3h |
| **TextEditable** | New `control:TextBox "Username"` | `IsReadOnly`, `MaxLength` | `set_text(value)` | New trait `TextEditablePattern` | ~2–4h |
| **TextSelection** | On TextBox element | `CaretPosition`, `SelectionRanges` | `select_range(start, end)` | New trait `TextSelectionPattern` | ~2–4h |
| **Scrollable** | List or new ScrollArea | `HorizontalPercent`, `VerticalPercent`, `CanScrollHorizontally`, `CanScrollVertically` | `scroll_to(h%, v%)` | New trait `ScrollablePattern` | ~2–4h |
| **StatefulValue** | New `control:Slider "Volume"` | `CurrentValue`, `Minimum`, `Maximum`, `SmallChange` | `set_value(v)` | New trait `StatefulValuePattern` | ~1–3h |
| **Activatable** | Buttons "OK", "Cancel" | `IsActivationEnabled` | `invoke()` | New trait `ActivatablePattern` | ~1h |
| **ItemContainer** | List, Tree, ComboBox | `ItemCount`, `IsVirtualized` | — (attributes only) | No new trait needed | ~30min |

**Total Effort Estimate:** ~15–30h for all patterns, easily parallelizable.

**Supplementary Elements for the Mock Tree (`mock_tree.xml`):**

```
app:Application "Mock Application"
├── Window "Operations Console" (Focusable, WindowSurface)
│   └── Panel "Workspace"
│       ├── List "Task List"                       (+ SelectionProvider, ItemContainer, Scrollable)
│       │   ├── ListItem "Analyze Project Status"   (+ Selectable)
│       │   ├── ListItem "Run Tests"                (+ Selectable, IsSelected=true)
│       │   ├── ListItem "Generate Report"           (+ Selectable)
│       │   └── ListItem "Validate Results"          (+ Selectable)
│       ├── Tree "Navigation"                       (+ ItemContainer)
│       │   ├── TreeItem "Dashboard"                (+ Expandable, IsExpanded=true)
│       │   │   ├── TreeItem "Overview"              (+ Selectable)
│       │   │   └── TreeItem "Metrics"               (+ Selectable)
│       │   └── TreeItem "Reports"                  (+ Expandable, IsExpanded=false)
│       │       └── …                               (Children only visible when expanded)
│       ├── TextBox "Username"                      (NEW: TextEditable, TextSelection, Text="")
│       ├── Slider "Volume"                         (NEW: StatefulValue, Value=50, Min=0, Max=100)
│       ├── Text "Status" (text: "Ready")
│       ├── Button "OK"                             (+ Activatable)
│       └── Button "Cancel"                         (+ Activatable)
└── Window "Detail View"
    └── Text "Description"

app:Application "Mock Settings"
└── Window "Settings"
    ├── CheckBox "Send reports automatically"       (+ Toggleable, ToggleState=Off)
    └── ComboBox "Theme"                            (+ SelectionProvider)
        ├── ComboBoxItem "Light"                    (+ Selectable)
        ├── ComboBoxItem "Dark"                     (+ Selectable, IsSelected=true)
        └── ComboBoxItem "System"                   (+ Selectable)
```

**What Still Cannot Be Tested via Mock (Even After Extension):**

| Area | Reason |
|---|---|
| Real input processing | Mock keyboard/pointer only log — the target app does not actually react to input |
| Timing / wait strategies | Mock responds instantly — real poll/wait scenarios require a live app |
| Provider events (live) | Mock events are simulated, not real OS events |
| Platform-specific quirks | UIA COM lifecycle, AT-SPI2 D-Bus timing, AX permissions |

The planned mock extensions would raise mock test coverage from ~70% to ~95% of keywords and patterns. The remaining ~5% can only be tested with a live app by design.

## 2. Test Layer Model (Target)

```
Layer 5: RF Acceptance (Live Test App)     ← Platform-specific, real UI, CI
Layer 4: RF Integration (Mock)             ← Cross-platform, deterministic
Layer 3: Python Unit Tests                 ← BareMetal keywords against mock
Layer 2: Rust Integration Tests            ← Provider contract tests, CLI (mock)
Layer 1: Rust Unit Tests                   ← Pure logic (XPath, types, parsing)
```

### 2.1 What Can Run Everywhere?

| Test Category | Platform | Approach |
|---|---|---|
| XPath (Rust) | All | Already done, pure logic |
| Runtime + CLI Mock (Rust) | All | Already done, mock-based |
| Python Bindings (Mock) | All | Already done, mock-based |
| BareMetal Keywords (Mock) | All | **New: pytest with `use_mock=True`** |
| RF Integration (Mock Tree) | All | **New: `.robot` suites with mock provider** |
| Provider Contract Tests | All | **New: Shared suite, mock + live provider** |

### 2.2 What Is Platform-Dependent?

| Test Category | Platform | Why |
|---|---|---|
| UIA Provider Smoke Tests | Windows | Requires real UIA COM API |
| AT-SPI2 Provider Smoke Tests | Linux | Requires D-Bus + AT-SPI2 |
| macOS AX Provider Smoke Tests | macOS | Requires NSAccessibility API |
| X11 Keyboard/Pointer Injection | Linux (X11) | Requires X11 + XTEST |
| Wayland Keyboard/Pointer Injection | Linux (Wayland) | Requires Wayland compositor + input protocols (not yet implemented) |
| Windows Keyboard/Pointer Injection | Windows | Requires SendInput |
| WindowManager (EWMH) | Linux (X11) | Requires EWMH-compatible WM |
| WindowManager (Win32) | Windows | Requires Win32 API |
| Live RF Acceptance (Test App) | Per OS CI Runner | Requires running test app + display |

## 3. Test Application — Options

### 3.1 Requirements

A test application must:
- Run on Windows, Linux, and macOS
- Be recognized by the respective accessibility APIs (UIA, AT-SPI2, macOS AX)
- Provide deterministic UI elements (buttons, text boxes, lists, trees, etc.)
- Be startable and automatable in CI (headless or virtual display)

### 3.2 Framework Evaluation

| Framework | Windows (UIA) | Linux (AT-SPI2) | macOS (AX) | CI-Capable | Effort | Notes |
|---|---|---|---|---|---|---|
| **egui (Rust, [AccessKit](https://github.com/AccessKit/accesskit))** | Yes (AccessKit→UIA) | Yes (AccessKit→AT-SPI) | Yes (AccessKit→AX) | Trivial (Rust binary) | Low | Already in the project (eframe 0.33); ~18 widget types exposed, but no TreeItem, Menu, Dialog, Tab |
| **Qt 6 / PySide6** | Excellent (UIA Bridge) | Excellent (AT-SPI2) | Good (AX) | Yes (Xvfb on Linux) | Medium | Best native a11y; new dependency |
| **GTK 4 / PyGObject** | Poor (no UIA) | Excellent (AT-SPI2) | Fair | Yes (Xvfb) | Low | Only useful on Linux |
| **Avalonia (C#)** | Good (UIA) | New (AT-SPI2, PR #20735, Feb 2026) | Good (AX) | Medium (.NET required) | Medium | .NET dependency; Linux AT-SPI2 just recently merged |
| **Tkinter (Python stdlib)** | Poor (MSAA, no UIA) | No AT-SPI2 | Rudimentary AX | Easy | Low | Poor accessibility — not recommended |
| **wxPython** | Good (UIA) | Fair (AT-SPI2) | Good (AX) | Medium | Medium | Possible alternative |
| **Electron / Tauri** | Good (Chromium→UIA) | Good (Chromium→AT-SPI2) | Good (Chromium→AX) | Difficult | High | Too heavyweight |

### 3.3 Proposed Approach: Two Tiers

#### Tier 1: egui Test App (immediately implementable)

egui is already used in the Inspector (`eframe = "0.33"`, see `apps/inspector/Cargo.toml`).
[AccessKit](https://github.com/AccessKit/accesskit) has been integrated as a default feature since [eframe 0.20.0 (Dec 2022)](https://github.com/emilk/egui/blob/main/crates/eframe/CHANGELOG.md#0200---2022-12-08---accesskit-integration-and-wgpu-web-support) ([PR #2294](https://github.com/emilk/egui/pull/2294)) and automatically exposes egui widgets via native accessibility APIs:
- **Windows:** [UI Automation](https://docs.rs/accesskit_windows) via `accesskit_windows`
- **Linux:** [AT-SPI](https://docs.rs/accesskit_unix) via `accesskit_unix`
- **macOS:** [NSAccessibility](https://docs.rs/accesskit_macos) via `accesskit_macos`

The integration uses the [AccessKit winit adapter](https://crates.io/crates/accesskit_winit), which is built into eframe.
For the complete list of all AccessKit roles see [`accesskit::Role`](https://docs.rs/accesskit/latest/accesskit/enum.Role.html) (182 variants).

**Advantages:**
- Dependency already present, team is familiar with the framework
- Rust-native, fits the project architecture
- No additional build step — just an `apps/testapp` crate
- Cross-platform accessibility out-of-the-box via AccessKit

**Limitations:**
- Accessibility tree reflects the `Ui` nesting (since [PR #7386](https://github.com/emilk/egui/pull/7386), included in 0.33: each `Ui` creates a `GenericContainer` node), but is flatter than in native toolkits
- No real native controls (custom rendering; egui renders everything itself, AccessKit only exposes the virtual accessibility structure)
- Some roles are missing in the egui mapping: no `TreeItem` (CollapsingHeader is exposed as `Button`), no `Menu`/`MenuItem`/`MenuBar`, no `Dialog`, no `Tab`/`TabList`, no `ListBox`/`ListItem` with selection
- Not representative of actual target applications (WPF, Qt, GTK, Electron)
- Immediate mode = constant tree updates, which can lead to many change events (AccessKit optimizes this via [incremental tree updates](https://github.com/AccessKit/accesskit#how-it-works))

**Available Widget Types:** egui exposes ~18 widget types (Button, TextEdit, Checkbox, RadioButton, ComboBox, Slider, DragValue, Label, Link, ProgressBar, Image, CollapsingHeader, ScrollArea, Panel, Window, ColorPicker, SelectableLabel) mapped to AccessKit roles. Notable gaps: no `TreeItem` (CollapsingHeader → `Button`), no `Menu`/`MenuItem`/`MenuBar`, no `Dialog`, no `Tab`/`TabList`, no `ListBox`/`ListItem` with selection. `egui_extras::Table` is a layout mechanism without table-specific AccessKit roles.

**Accessibility API:** egui offers three levels of accessibility customization — from automatic role mapping (standard widgets), to full `accesskit::Node` control via `Context::accesskit_node_builder()`, to explicit parent overrides via `UiBuilder::accessibility_parent()`. Since [PR #7386](https://github.com/emilk/egui/pull/7386) (included in 0.33), parent-child hierarchies are automatically derived from `Ui` nesting. For `AutomationId` support, AccessKit's `author_id` property maps directly to UIA `AutomationId`. A custom `AccessKitExt` convenience trait is recommended for the test app.

> For the complete widget/role mapping table, all three API levels with code examples, Widget IDs, `AutomationId`, the `AccessKitExt` trait proposal, and the API level quick reference, see **[egui Accessibility API Guide](egui-accessibility-guide.md)**.

**CI Display Requirements:**
- **Linux (X11):** `Xvfb :99 &` + `DISPLAY=:99` — egui/eframe renders via the virtual framebuffer.
- **Linux (Wayland):** Headless compositor (e.g., `weston --backend=headless` or `wlheadless-run`). Not yet a priority since the project currently targets X11.
- **Windows:** Runs natively — GitHub Actions Windows runners have a display.
- **macOS:** Runs natively — GitHub Actions macOS runners have a display. May require screen recording permissions for accessibility.

**Implementation:**
```
apps/testapp/
├── Cargo.toml
└── src/
    └── main.rs    ← egui window with standard widgets
```

#### Tier 2: Qt/PySide6 Test App (medium-term)

For representative tests against real native OS controls:

**Advantages:**
- Qt widgets are natively recognized by UIA/AT-SPI2/AX
- Full pattern coverage (~90%): Selectable, Toggleable, Expandable, Scrollable, Menu, Tab, Dialog
- Closer to real target applications
- `uv add --dev PySide6` — clean integration into the build system

**Planned Widgets:**

| Qt Widget | Accessible Name | Test Purpose |
|---|---|---|
| `QPushButton` | "OK", "Cancel" | Click, Focus, Bounds, ActivationPoint |
| `QLineEdit` | "Username" | TextContent, TextEditable, Keyboard Type |
| `QTextEdit` | "Notes" | Multi-line Text, Selection |
| `QListWidget` (4 items) | "Tasks" | List/ListItem, Selectable |
| `QTreeWidget` (nested) | "Navigation" | Tree/TreeItem, Expandable |
| `QCheckBox` | "Enable Logging" | Toggleable |
| `QComboBox` (3 items) | "Theme" | ComboBox, Selection |
| `QTabWidget` (2 tabs) | "Settings" / "Output" | Tab/TabItem |
| `QLabel` | "Status: Ready" | TextContent (read-only) |
| `QMenuBar` (File, Edit) | Menu/MenuItem | Menu navigation |
| `QSlider` | "Volume" | StatefulValue |

**Implementation:**
```
tests/testapp/
├── main.py          ← PySide6 window
└── requirements.txt ← or as dev dependency in pyproject.toml
```

**CI Integration:**
- Linux: `Xvfb :99 &` + `DISPLAY=:99`
- Windows/macOS: runs natively

### 3.4 Comparison

| Criterion | egui (Tier 1) | Qt/PySide6 (Tier 2) |
|---|---|---|
| Effort | Low (already present) | Medium (new dependency) |
| Cross-Platform A11y | Yes | Yes |
| Representativeness | Low (custom rendering) | High (native controls) |
| Pattern Coverage | ~18 widget types (no TreeItem, Menu, Dialog, Tab) | ~90% (full) |
| CI Integration | Trivial (Rust binary) | Medium (Python + Xvfb) |
| Value for Provider Tests | Smoke tests | Full regression |

## 4. Implementation Plan

### Phase 1: Close Mock-Based Gaps (no new tooling needed)

| Item | What | Priority |
|---|---|---|
| Python BareMetal Unit Tests | pytest tests for all keywords against mock provider (`use_mock=True`): Query, Focus, Pointer Click, Keyboard Type, Get Attribute, Highlight, Screenshot, etc. | High |
| RF Integration Tests (Mock) | `.robot` suites under `tests/` with `use_mock=${true}` — deterministic XPath queries, attribute assertions, keyword integration. Run in CI on all platforms. | High |
| Provider Contract Tests (Rust) | Shared test functions in `core::ui::contract::testkit` run against mock provider AND live providers. Checks: attribute completeness, pattern consistency, namespace correctness. | Medium |
| `platynui-link` Tests | Simple smoke tests for the linking macros. | Low |

### Phase 2: Cross-Platform CI (parallel to Phase 1)

| Item | What | Priority |
|---|---|---|
| Windows CI Runner | GitHub Actions Windows runner, `cargo nextest run`, Python tests. Enables existing Windows tests + future UIA tests. | High |
| macOS CI Runner | GitHub Actions macOS runner, at least build + basic tests. | Medium |
| RF Mock Tests in CI | `uv run robot tests/` in CI on all platforms. | High |

### Phase 3: Test Applications (medium-term)

| Item | What | Priority |
|---|---|---|
| egui Test App (Tier 1) | `apps/testapp` with basic widgets. Smoke tests for provider basics: Focus, Bounds, ActivationPoint, TextContent. | High |
| Platform Smoke Suites | RF suites that start the egui app, find elements, interact, verify attributes. Tags: `platform:windows`, `platform:linux`, `platform:all`. | High |
| Qt Test App (Tier 2) | PySide6 window with complete widget set for comprehensive pattern tests. | Medium |
| Provider Integration Tests | Rust integration tests that interact with the test apps (platform-specific in CI). | Medium |

### Phase 4: Extend Mock Provider & Quality

Mock extension can also be tackled **before the test apps** (Phase 3) — it requires no external dependencies and immediately maximizes mock-based test coverage. Detailed plan in §1.5.

**Recommended Order** (by impact on test coverage):

| Item | What | Priority |
|---|---|---|
| Mock: Toggleable | `ToggleablePattern` trait in core + `toggle.rs` in mock. CheckBox gets dynamic `ToggleState`. → Enables: CheckBox tests in RF/pytest. | High |
| Mock: Selectable + SelectionProvider | Traits in core + `select.rs` in mock. ListItems/ComboBoxItems become selectable, List/ComboBox become SelectionProviders. → Enables: list selection, multi-selection. | High |
| Mock: Expandable | Trait in core + `expand.rs` in mock. TreeItems get `IsExpanded` state, `expand()`/`collapse()` actions. → Enables: tree navigation. | High |
| Mock: Activatable | Trait in core + `activate.rs`. Buttons become invokable. → Enables: button click tests without pointer. | Medium |
| Mock: TextEditable + TextSelection | Traits in core + new TextBox element in XML. Writable text field with cursor/selection. → Enables: complete text input tests. | Medium |
| Mock: Scrollable | Trait in core + `scroll.rs`. List/new ScrollArea gets scroll position. → Enables: scroll tests. | Medium |
| Mock: StatefulValue | Trait in core + new Slider element in XML with Value/Min/Max. → Enables: range/slider tests. | Medium |
| Mock: ItemContainer (attribute-only) | No new trait. `ItemCount`/`IsVirtualized` as attributes on List/Tree/ComboBox. | Low |
| Mock Tree: new XML elements | `control:TextBox "Username"` + `control:Slider "Volume"` into `mock_tree.xml`. | Medium |
| Performance Benchmarks | XPath + tree traversal benchmarks in CI (regression tracking). | Medium |
| Coverage Reporting | `cargo llvm-cov` + `pytest-cov` in CI. | Low |
| Snapshot/Golden Tests | CLI `snapshot` output compared against stored reference XMLs. | Medium |

## 5. Target Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  RF Acceptance Suites (Live Test Apps: egui + Qt)                │
│  ├── tests/acceptance/common/*.robot  (all platforms)            │
│  ├── tests/acceptance/windows/*.robot (UIA-specific)             │
│  ├── tests/acceptance/linux/*.robot   (AT-SPI2-specific)         │
│  └── tests/acceptance/macos/*.robot   (AX-specific)              │
├──────────────────────────────────────────────────────────────────┤
│  RF Integration Suites (Mock Provider, deterministic)            │
│  └── tests/integration/*.robot  (all platforms, CI)              │
├──────────────────────────────────────────────────────────────────┤
│  Python pytest (BareMetal Keywords + Bindings)                   │
│  ├── packages/native/tests/  (binding tests, mock)               │
│  └── tests/python/  (BareMetal keyword tests, mock)              │
├──────────────────────────────────────────────────────────────────┤
│  Rust Tests (Unit + Integration, ~1,847 existing)                │
│  ├── crates/xpath/tests/  (XPath engine, pure logic)             │
│  ├── crates/*/src/  (#[cfg(test)] modules, per crate)            │
│  ├── crates/core/tests/  (contract tests)                        │
│  └── crates/runtime/src/test_support.rs  (shared fixtures)       │
└──────────────────────────────────────────────────────────────────┘
```

## 6. Decisions and Open Questions

### 6.1 Decided

The following questions from earlier iterations have been resolved:

| Question | Decision | Rationale |
|---|---|---|
| **PySide6 vs. PyQt6?** | **PySide6** | LGPL license (vs. GPL for PyQt6), functionally equivalent. |
| **Platform-specific tags/skips in RF?** | Standard RF tagging: `[Tags]  platform:windows` | Use `robot --include platform:windows` or `--exclude` for selective execution. |
| **`robot.toml` for suite configuration?** | Extend existing `robot.toml` (already in project root) | Currently configured with `paths = "tests"`. Extend with variable files, tag filters, and output settings for mock vs. live suites. |
| **Timing/waits in RF tests?** | Start with poll-based (`Wait Until Keyword Succeeds`) | Mock tests respond instantly. For live tests, polling is pragmatic. Event-based waiting can be added later as an optimization. |

### 6.2 Open

- **Should the egui test app be packaged as a Python wheel** (analogous to CLI/Inspector)? This would allow RF suites to start it via `platynui-testapp`. Trade-off: additional maturin build step vs. simpler test orchestration.
- **How exactly should the test app be started and stopped in CI?** Likely approach: RF `Process` library or `subprocess` in `conftest.py`, start as background process, terminate by PID after test run. Needs a concrete spike.
