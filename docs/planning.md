# PlatynUI Planning & Roadmap

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document tracks all open work items, decided-but-not-implemented designs, open design questions, and the complete task history. For the current architecture, see `docs/architecture.md`.

## 1. Cross-Cutting Guidelines

- **Dependencies**: Always use the currently stable published version. Use `cargo search`, crates.io, or `cargo outdated` and mention version status in reviews.
- **Crate naming**: Workspace crates (`crates/*`, `apps/*`) must use `platynui-` package name prefix. Packaging/FFI crates outside the workspace follow target ecosystem conventions.
- **Tests**: Use `rstest` consistently (fixtures, `#[rstest]` with `case`/`matrix`, parametric tests). Migrate existing tests when touching them.
- **Type naming**: No `Platynui` prefix on Rust types. Use descriptive names within modules (`RuntimeState`, `WindowsDeviceBundle`).
- **Plan maintenance**: Update this document after each work package — check off completed tasks, add new findings.
- **Mock stack**: `mock-provider` feature gates mock platform/provider. Standard builds exclude mocks. Productive crates are linked by applications via `cfg(target_os = ...)` + `platynui-link` macros.
- **Rust toolchain**: `rustc 1.90.0` baseline. Track release notes and adopt new features as appropriate.
- **Lints**: `unsafe_code = "forbid"`, `unused_must_use = "deny"`, `clippy::pedantic = "warn"`, `clippy::non_std_lazy_statics = "deny"`.

## 2. Current Status Summary

### Fully Implemented
- Core type system and trait definitions
- XPath 2.0 parser, compiler, and streaming evaluator
- Runtime orchestration, provider registry, event pipeline
- Windows platform (pointer, keyboard, screenshot, highlight, desktop info, window manager)
- Windows UIA provider (COM/MTA, Raw View Walker, patterns, native properties)
- Linux X11 platform (pointer, keyboard, screenshot, highlight, desktop info)
- Linux AT-SPI2 provider (D-Bus, role mapping, Focusable pattern, component-gated attributes)
- Mock platform and provider (full test infrastructure)
- CLI with all major commands
- Inspector GUI (egui MVVM: tree view, properties, XPath search, highlighting)
- Python bindings (core types, runtime, evaluation, pointer, keyboard, patterns)
- Dynamic linking framework (`platynui-link`)
- CI pipeline (Linux/Windows/macOS: build, test, format, lint, wheels)

### Stubs / Not Yet Implemented
- macOS platform (`crates/platform-macos`) — marker type only
- macOS AX provider (`crates/provider-macos-ax`) — minimal factory, empty iterators
- Event-driven cache invalidation

## 3. Decided Design — Not Yet Implemented

### 3.1 Event-Driven Cache Invalidation

The current XDM cache (Option A: Lazy Revalidation) detects removed nodes via `is_valid()` but does NOT detect newly-added children of still-valid parents until the cache is manually cleared.

**Decided design** (Option B): Central dirty flag in `Runtime`:

```rust
pub struct Runtime {
    cache_dirty: AtomicBool,  // Set by event listeners on structural changes
}
```

Before each cached evaluation: `if cache_dirty.swap(false, Acquire) { cache.clear() }`. RuntimeEventListener sets the flag on `NodeAdded`, `NodeRemoved`, `TreeInvalidated`. Attribute updates call `node.invalidate()` instead.

**Tasks:**

- [ ] **Core dirty flag**: Add `cache_dirty: AtomicBool` to `Runtime`. `RuntimeEventListener` sets flag on `NodeAdded`, `NodeRemoved`, `TreeInvalidated`; calls `node.invalidate()` for `NodeUpdated`. Before cached evaluation: `if cache_dirty.swap(false, Acquire) { cache.clear() }`. Descriptor: set `ProviderEventCapabilities::STRUCTURE`.

- [ ] **Windows UIA structure events**: Implement `IUIAutomationStructureChangedEventHandler` (COM callback). Subscribe via `AddStructureChangedEventHandler(element, TreeScope_Subtree, handler)`. Map all change types (`ChildAdded`, `ChildRemoved`, `ChildrenReordered`, `ChildrenBulkAdded`, `ChildrenBulkRemoved`, `ChildrenInvalidated`) to `TreeInvalidated`. COM MTA events arrive on arbitrary thread → `AtomicBool` is thread-safe. Shutdown: remove handlers BEFORE releasing `IUIAutomation`.

- [ ] **AT-SPI2 D-Bus structure events**: Subscribe to `object:children-changed:add` and `object:children-changed:remove` on the accessibility bus. Background async task with `zbus` signal stream, forward as `TreeInvalidated`. Optional: map with parent path to granular `NodeAdded`/`NodeRemoved`. Shutdown: drop subscription before closing D-Bus connection.

- [ ] **macOS AX structure events**: Register `AXObserver` with `AXObserverAddNotification` for `kAXCreatedNotification` → `TreeInvalidated`, `kAXUIElementDestroyedNotification` → `TreeInvalidated`, `kAXValueChangedNotification` → `NodeUpdated` (attribute invalidation only). Run observer on CFRunLoop. Shutdown: `AXObserverRemoveNotification` + stop run loop.

- [ ] **Granular property events (optional)**: Subscribe to property change events per platform (UIA `PropertyChangedEventHandler`, AT-SPI `object:property-change`, macOS `kAXValueChangedNotification`). Map to `NodeUpdated` with targeted `node.invalidate()`. Avoids full cache clear for attribute-only changes.

**Open questions:**
- Event debouncing needed for high-frequency changes? (e.g., scrolling triggers many `ChildrenReordered`)
- Handler scope: `TreeScope_Subtree` from Desktop or specific context node?
- Granular tree invalidation vs. blanket `TreeInvalidated`?
- Shutdown ordering: COM handlers before `IUIAutomation` release? (critical for Windows)

### 3.2 Virtual Desktop Switching

Design decided: `ensure_window_accessible()` method on `WindowManager` trait.

- [ ] X11 EWMH: read `_NET_WM_DESKTOP`, switch via `_NET_CURRENT_DESKTOP` ClientMessage (~30 lines)
- [ ] Windows: `IVirtualDesktopManager::GetWindowDesktopId` + `MoveWindowToDesktop` (COM API)
- [ ] macOS: no-op (already handled by `kAXRaiseAction`)
- [ ] Integration: `bring_to_front()` calls `ensure_window_accessible()` before `activate()`, best-effort with `warn!` on error

### 3.3 ScrollIntoView Runtime Action

- [ ] Define `scroll_into_view(node: &Arc<dyn UiNode>)` as runtime function for all elements
- [ ] `ScrollIntoViewPattern`: providers implement for every element, not just scrollable ones
- [ ] Dynamic container detection: runtime traverses ancestor chain for `Scrollable` containers
- [ ] Mock implementation: scrollable container hierarchies + runtime logging
- [ ] UIA implementation: `ScrollItemPattern::ScrollIntoView()` + `VirtualizedItemPattern::Realize()`
- [ ] CLI integration: `--scroll-into-view` flag for `pointer click`, separate subcommand
- [ ] Tests and documentation

### 3.4 Out-of-Process Provider Support (Future)

Idea: allow external processes to act as UI tree providers via a JSON-RPC-like protocol (inspired by LSP/MCP). This would enable third-party integrations without compiling Rust code. No design has been finalized; this is a wishlist item.

- [ ] Define protocol contract (transport, handshake, node API, events)
- [ ] Implement provider adapter crate
- [ ] Implement server facade for remote runtime access
- [ ] Example provider + third-party guide

## 4. Platform Completion

### 4.1 Linux X11 — Remaining Items

- [x] **Keyboard device** (`platynui-platform-linux-x11`):
  - XTest injection (`FakeKeyEvent`) with keysym-to-keycode resolution via `GetKeyboardMapping`
  - Named key table (~120 entries, case-insensitive): modifiers, function keys, navigation, numpad, symbol aliases
  - Single-character resolution via keysym mapping with CapsLock-aware shift management
  - Dynamic keycode remapping for characters outside the active layout via `ChangeKeyboardMapping`
  - Control character mapping (`\n`→Return, `\t`→Tab, `\r`→Return, BS→Backspace, ESC→Escape, DEL→Delete)
  - Symbol aliases: `PLUS`, `MINUS`, `LESS`/`LT`, `GREATER`/`GT` (consistent with Windows/Mock)
  - `start_input()`: refreshes keyboard mapping at start of each input session
  - No external dependency on `xkbcommon-rs` — uses pure `x11rb` `GetKeyboardMapping` instead
- [x] **EWMH WindowManager migration**: moved `ewmh.rs` from `provider-atspi` to `platform-linux-x11` as `window_manager.rs`, registered as `WindowManager`
  - [x] `resolve_window()`: PID from parent chain `control:ProcessId` + `_NET_CLIENT_LIST` + `_NET_WM_PID` matching, disambiguate via `_NET_WM_NAME`
  - [x] EWMH support check in `PlatformModule::initialize()`: `_NET_SUPPORTING_WM_CHECK`, `_NET_SUPPORTED` atoms
  - [ ] `ensure_window_accessible()`: read `_NET_WM_DESKTOP`, switch via `_NET_CURRENT_DESKTOP` ClientMessage
- [x] **Extended EWMH**: `_NET_WM_STATE` (minimize/maximize), `_NET_MOVERESIZE_WINDOW` (move/resize)
- [x] **Provider migration** (`provider-atspi`): removed `ewmh.rs`, removed `x11rb` dependency, replaced window calls with `WindowManager` trait
- [ ] **AT-SPI2 events**: `subscribe_events` implementation (D-Bus signal handling)
- [ ] **AT-SPI2 tree verification**: confirm Application → Window → Control/Item structure
- [ ] **Smoke tests**: desktop bounds, ActivationPoint, visibility/enable flags under X11
- [ ] **CLI `window` on X11**: end-to-end window listing and actions
- [ ] Focus helper for AT-SPI2 + platform fallbacks
- [ ] Screenshot performance: optional XShm acceleration
- [ ] Highlight UX improvements (seamless, centered labels)

### 4.2 Windows — Remaining Items

- [ ] Watch filters (`--namespace`, `--pattern`, `--runtime-id`) for CLI `watch`
- [ ] UIA smoke tests: iterator order, app attributes, native properties
- [ ] Keyboard error mapping refinement (Win32 `LastError`)
- [ ] Keyboard tests: AltGr on DE layout, CapsLock scenarios, extended keys, VK special groups (OEM, ABNT, DBE)
- [ ] Optional: `VkKeyScanExW` with thread layout (HKL) for multi-layout scenarios
- [ ] `ensure_window_accessible()` via `IVirtualDesktopManager` COM API
- [ ] Extended error handling: foreground locks, UAC dialogs, non-responsive apps
- [ ] Integration tests: WPF, WinForms, Win32, UWP
- [ ] WindowSurface documentation: flow diagrams, troubleshooting

### 4.3 macOS — Full Platform (Stub → Implementation)

- [ ] `platynui-platform-macos`: Devices via Quartz/CoreGraphics (CGEvent), Event Taps for keyboard, `CGDisplayCreateImage` for screenshot, transparent `NSWindow`/CoreAnimation for highlight
- [ ] `platynui-provider-macos-ax`: `AXUIElement` bridge, window/app listing, RuntimeId from `AXIdentifier`, bounds conversion (Core Graphics), role/subrole mapping
- [ ] WindowManager via AppKit (`AXUIElement`, `CGWindowListCopyWindowInfo`)
- [ ] Cross-platform regression tests for macOS-specific differences

### 4.4 Wayland (Future)

**Current status:** PlatynUI uses X11/XWayland exclusively. Wayland support is deferred until the protocol landscape matures.

**Mediation crate (`platynui-platform-linux`):**
- [ ] Create mediation crate that bundles `platform-linux-x11` and `platform-linux-wayland`
- [ ] Runtime session detection via `$XDG_SESSION_TYPE` (`x11` / `wayland`)
- [ ] Fallback: attempt X11 first for XWayland compatibility
- [ ] Register via standard `inventory` mechanism

**Platform devices (`platynui-platform-linux-wayland`):**
- [ ] PointerDevice via Wayland protocols (compositor-dependent)
- [ ] KeyboardDevice via Wayland protocols
- [ ] ScreenshotProvider (compositor-dependent, e.g. `wlr-screencopy` or PipeWire)

**WindowManager — Protocol landscape:**

See `docs/architecture.md` §8.5 for the full Wayland protocol assessment table. In summary: no universal protocol exists. The strategy is to start with `wlr-foreign-toplevel-management` (Sway/Hyprland), add compositor-specific backends as needed, and fall back to no-op where unsupported.

**Implementation effort:** High to very high due to compositor-specific fragmentation.

- [ ] WindowManager via `wlr-foreign-toplevel-management` (Sway/Hyprland)
- [ ] Evaluate KDE D-Bus backend for Plasma
- [ ] Evaluate COSMIC workspace protocol
- [ ] `ensure_window_accessible()` best-effort with graceful fallback

## 5. Tool & DX Completion

### 5.1 Inspector

Remaining work:
- [x] **Streaming XPath search** (see §5.1.1 below)
- [x] **TreeView widget component** (see §5.1.2 below)
- [ ] **Element Picker**: click-to-identify mode (click on screen element → reveal in tree)
- [ ] **Export**: copy XPath for selected node, export subtree as XML
- [ ] **Performance**: measurement with large trees (≥2k visible nodes), virtual scrolling (part of TreeView widget)
- [ ] **Async child loading**: non-blocking tree expansion for slow providers (AT-SPI2 D-Bus, large UIA trees)
- [ ] **Filter/search in properties**: quick filter for attribute names

#### 5.1.1 Streaming XPath Search — Design

**Problem:** `evaluate_xpath()` calls `runtime.evaluate()` synchronously, which materializes
ALL results before returning. For large trees or expensive XPath expressions, this blocks the
GUI thread and makes the application unresponsive.

**Goal:** Non-blocking, streaming XPath evaluation with:
1. UI remains responsive during search
2. Results appear incrementally as they are found
3. Search can be cancelled at any time
4. Live status display (elapsed time, result count, spinner)

**Technical constraints:**
- `EvaluationStream` is `!Send` (uses `Rc`-based XDM nodes internally) — evaluation must
  happen entirely on the spawned thread, not moved back to the GUI thread.
- `EvaluationItem` IS `Send` (contains `Arc<dyn UiNode>`, `UiValue`) — results can be sent
  via `mpsc::channel` to the GUI thread.
- `Runtime` is `Send + Sync` — can be cloned (`Arc<Runtime>`) and moved to the background thread.
- `DynamicContextBuilder::with_cancel_flag(Arc<AtomicBool>)` already exists in the XPath engine
  and is checked at each axis step.

**Design:**

1. **Runtime layer — Cancel flag plumbing:**
   - Add `cancel_flag: Option<Arc<AtomicBool>>` to `EvaluateOptions`.
   - Wire it into `EvaluationStream::new()` and `evaluate_iter()` via
     `DynamicContextBuilder::with_cancel_flag()`.
   - Add `Runtime::evaluate_iter_owned_cancellable(node, xpath, cancel_flag)` convenience method.

2. **Inspector ViewModel — Background search types:**
   ```rust
   enum SearchMsg {
       Result(EvaluationItem),
       Done { elapsed: Duration },
       Error(String),
       Cancelled,
   }

   struct ActiveSearch {
       receiver: mpsc::Receiver<SearchMsg>,
       cancel_flag: Arc<AtomicBool>,
       start: Instant,
       count: usize,
   }
   ```

3. **Inspector ViewModel — `evaluate_xpath()`:**
   - Cancel any existing `ActiveSearch` (set `cancel_flag`, drop receiver).
   - Clone `Arc<Runtime>`, clone XPath string.
   - Create `Arc<AtomicBool>` cancel flag, create `mpsc::channel`.
   - Spawn `std::thread::spawn` that:
     a. Calls `EvaluationStream::new(None, xpath, options_with_cancel_flag)`
     b. Iterates the stream, sending `SearchMsg::Result(item)` for each `Ok(item)`
     c. On `Err`, sends `SearchMsg::Error(err.to_string())` and stops
     d. After loop, sends `SearchMsg::Done { elapsed }` (or `Cancelled` if flag was set)
   - Store `ActiveSearch` in ViewModel.

4. **Inspector ViewModel — `poll_search()`:**
   - Called every frame from `update()`.
   - Drains up to 100 items per frame from `receiver` (debouncing).
   - For each `SearchMsg::Result`: convert to `SearchResultItem`, append to `results`.
   - For `SearchMsg::Done`: finalize `result_status` with count + elapsed time, clear `ActiveSearch`.
   - For `SearchMsg::Error`: set `result_status` with error, clear `ActiveSearch`.
   - For `SearchMsg::Cancelled`: set `result_status` "Search cancelled", clear `ActiveSearch`.
   - Update `result_status` live: "Searching… N results (X.Xs)" while active.
   - Call `ctx.request_repaint()` while search is active to keep UI polling.

5. **Inspector ViewModel — `cancel_search()`:**
   - Set `cancel_flag` to `true` (the XPath engine checks this at each axis step).
   - The background thread will see the flag, stop iterating, and send `Cancelled`.

6. **Toolbar UI changes (`toolbar.rs`):**
   - While `ActiveSearch` is active: show `⏹ Stop` button instead of `▶ Search`.
   - Add spinner character rotation (◐◓◑◒) to status line while searching.
   - New `ToolbarAction::CancelSearch` variant.
   - Auto-cancel: starting a new search while one is running cancels the previous one.

7. **App wiring (`lib.rs`):**
   - In `update()`, call `self.vm.poll_search(ctx)` before processing toolbar actions.
   - `poll_search` calls `ctx.request_repaint()` when a search is active to drive continuous polling.

**Tasks:**
- [x] Add `cancel_flag` to `EvaluateOptions` + wire into `EvaluationStream::new()` / `evaluate_iter()`
- [x] Add `Runtime::evaluate_iter_owned_cancellable()`
- [x] Implement `SearchMsg`, `ActiveSearch` in inspector ViewModel
- [x] Rewrite `evaluate_xpath()` to spawn background thread
- [x] Implement `poll_search()` and `cancel_search()`
- [x] Update toolbar: Stop button, spinner, `CancelSearch` action
- [x] Wire `poll_search()` into `lib.rs` `update()`
- [ ] Test with large trees and long-running XPath expressions

#### 5.1.2 TreeView Widget Component — Design

**Problem:** The current `show_tree()` is a free function tightly coupled to Inspector-specific
types (`VisibleRow`, `TreeAction`). Keyboard navigation lives separately in `lib.rs`. This makes
the tree neither reusable nor self-contained. Virtual scrolling — needed for large trees — would
be best implemented inside the component itself.

**Goal:** A generic, self-contained egui TreeView widget with:
1. Reusable across different tree data sources (UI tree, file tree, XPath result tree)
2. Built-in keyboard navigation (Up/Down/Left/Right/Home/End/PageUp/PageDown)
3. Virtual scrolling for large trees (only visible rows rendered)
4. Customizable icon/label rendering via callbacks

**Design — Builder pattern with trait + response struct (Option B):**

```rust
/// Trait that tree data rows must implement.
pub trait TreeRowData {
    fn label(&self) -> &str;
    fn depth(&self) -> usize;
    fn has_children(&self) -> bool;
    fn is_expanded(&self) -> bool;
    fn is_valid(&self) -> bool;
}

/// Builder for the TreeView widget.
pub struct TreeView<'a, R: TreeRowData> {
    rows: &'a [R],
    selected: Option<usize>,
    focused: usize,
    scroll_to_focused: bool,
    icon_fn: Option<Box<dyn Fn(&R) -> &str + 'a>>,
    context_menu_fn: Option<Box<dyn FnMut(&mut egui::Ui, usize) + 'a>>,
}

/// Response returned after rendering.
pub struct TreeResponse {
    /// Row that was clicked/selected, if any.
    pub selected: Option<usize>,
    /// Row whose expand/collapse was toggled, if any.
    pub toggled: Option<usize>,
    /// Keyboard navigation delta (consumed internally, reported for ViewModel sync).
    pub navigate: Option<TreeNavigate>,
}

pub enum TreeNavigate {
    Up, Down, Left, Right, Home, End, PageUp, PageDown,
}
```

**Usage:**
```rust
let response = TreeView::new(&rows)
    .selected(self.selected_index)
    .focused(self.focused_index)
    .scroll_to_focused(self.scroll_to_focused)
    .icon(|row| role_icon(&row.label))
    .context_menu(|ui, idx| { /* Refresh / Refresh Subtree */ })
    .show(ui);

if let Some(idx) = response.selected {
    self.vm.select_node(idx);
}
```

**Key design decisions:**
- `TreeRowData` trait decouples from `VisibleRow` — any type implementing the trait works.
- Icon mapping via callback (`icon_fn`) instead of hardcoded `role_icon()`.
- Context menu via callback — Inspector-specific actions stay in Inspector code.
- Keyboard handling moves INTO the widget (currently scattered in `lib.rs`).
- Virtual scrolling: use known row height + scroll offset to compute visible range,
  only iterate/render rows in that range. `scroll_to_me()` replaced by explicit offset calculation.
- `TreeResponse` carries structured actions instead of `Vec<TreeAction>`.

**Tasks:**
- [x] Define `TreeRowData` trait and `TreeResponse` struct
- [x] Implement `TreeView` builder with `show()` method
- [x] Move keyboard navigation from `lib.rs` into the widget
- [x] Implement virtual scrolling (fixed row height, visible range calculation)
- [x] Implement `VisibleRow: TreeRowData` in Inspector
- [x] Migrate Inspector `show_tree()` → `TreeView::new(...).show(ui)`
- [x] Remove old `show_tree()` function and `TreeAction` enum

### 5.2 CLI

- [ ] `dump-node` command
- [ ] Script integration / CLI ergonomics
- [ ] Example workflows documentation (XPath → Highlight, focus, window status)

### 5.3 Robot Framework Integration

The `PlatynUI.BareMetal` library provides low-level keywords backed by `platynui_native`. The main `PlatynUI` library is a scaffold (placeholder keywords only). Key work areas:

- [ ] **BareMetal keyword expansion**: additional keywords for patterns beyond Focusable/WindowSurface (TextContent, Selectable, Toggleable, Scrollable, Expandable)
- [ ] **Waiting strategies**: configurable poll-based waits for element existence, visibility, focus, and attribute values (with timeout + retry interval)
- [ ] **Locator syntax**: define how XPath expressions are passed to keywords — raw strings, aliases, or a locator DSL
- [ ] **High-level PlatynUI library**: keyword layer above BareMetal with element abstractions, implicit waits, and human-readable API
- [ ] **Acceptance test suites** under `tests/BareMetal/` covering core workflows (launch app, find element, interact, verify)
- [ ] **Documentation**: keyword reference, example `.robot` files, getting-started guide

## 6. XPath Optimization Backlog

From the XPath streaming analysis:

- [ ] Predicate pushdown — high impact, push filters closer to axis traversal
- [ ] `evaluate_first()` fast path — stop after first match
- [ ] Constant folding in compiler
- [ ] Streaming completeness: eliminate remaining `collect()` calls in string operations and sequence operators
- [ ] Memory profiling and benchmark suite
- [ ] Persistent XPath caching & snapshot layer (if performance demands)

## 7. Quality & Process

- [ ] Contract tests for providers & devices (pattern-specific attributes, desktop coordinates, RuntimeId sources)
- [ ] Release/versioning strategy (SemVer per crate? Workspace version?)
- [ ] UiNode `Id` tests: core contract tests, provider smoke tests (UIA, AT-SPI, macOS)
- [ ] `Id` mapping for AT-SPI2 (`accessible_id`) and macOS AX (`AXIdentifier`)
- [ ] CLI/Python example queries for `Id` documented

## 8. Open Design Questions

Status legend: **NEW** = not yet discussed, **DISCUSSED** = considered but no decision, **DEFERRED** = postponed intentionally.

1. **Event debouncing** — needed for high-frequency structural changes? Strategy? — **DISCUSSED** (see §3.1 open questions)
2. **UIA event scope** — `TreeScope_Subtree` from Desktop or specific context node? — **DISCUSSED** (see §3.1 open questions)
3. **macOS Space switching** — system setting detection for `kAXRaiseAction` implicit switch? — **DEFERRED** (macOS platform not yet implemented)
4. **Windows AUMID as Application Id** — prefer over process name? Via `SHGetPropertyStoreForWindow(hwnd)` → `PKEY_AppUserModel_ID`? — **NEW**
5. **Python custom exception hierarchy** — extend beyond current set? — **NEW**
6. **Provider event subscription in Python** — how to expose? — **DEFERRED** (event pipeline not yet exposed to Python)
7. **Pattern versioning** — needed? How to handle evolution? — **DEFERRED** (premature during preview phase)
8. **Coordinate system consistency** — DPI/scaling/multi-monitor edge cases? — **DISCUSSED** (Per-Monitor-V2 on Windows done; Linux/macOS TBD)

## 9. Backlog & Explorations

- Additional patterns: table navigation, drag & drop
- Extended input devices: gamepad, stylus, touch
- Touch device support (traits, CLI commands)
- Community guides, example providers, training material
- Optional provider runtime services interface
- Persistent XPath caching & snapshot layer

---

## 10. Full Task Tracking

Complete checklists from all work areas, including completed items for historical reference.

### 10.1 Foundation & Repository Structure

- [x] Workspace setup: `crates/core`, `crates/runtime`, `crates/server`, platform/provider crates, `crates/cli`, `apps/inspector`
- [x] Common Cargo settings (edition, lints, features), rustfmt/clippy configuration
- [x] README/CONTRIBUTING updated with naming conventions, architecture overview
- [x] Dev tooling documented (`uv`, `cargo`, Inspector dependencies), base scripts

### 10.2 Core Data Model & XPath

- [x] `UiNode`/`UiAttribute` traits introduced, old struct/builder approach removed
- [x] Runtime wrappers for `UiNode`/`UiAttribute` (direct `Arc<dyn UiNode>` adapters)
- [x] `UiPattern` base trait + `UiNode::pattern::<T>()` lookup + `PatternRegistry`
- [x] `UiValue` defined (structured values: Rect, Point, Size, Integer, JSON conversions)
- [x] Namespace registry (`control`, `item`, `app`, `native`) + helpers
- [x] Evaluation API on `EvaluationItem` (Node/Attribute/Value)
- [x] Base validator `validate_control_or_item` (duplicate `SupportedPatterns` check)
- [x] Desktop document node (monitors, bounds as Rect, alias attributes)
- [x] XPath atomization on `typed_value()`: UiValue → XDM atomics

### 10.3 Pattern System

- [x] Runtime pattern traits (`FocusablePattern`, `WindowSurfacePattern`) + `PatternError`
- [x] Runtime action interfaces (via `FocusableAction`, `WindowSurfaceActions` + rstest coverage)
- [x] `SupportedPatterns` usage guide
- [x] Provider-facing contract tests (`platynui_core::ui::contract::testkit`)
- [x] Mapping helpers between patterns and technology-specific APIs
- [x] Patterns document updated (alias attributes clarification, role catalog, mapping tables)

### 10.4 Provider Infrastructure (Core)

- [x] Traits: `UiTreeProvider`, `UiTreeProviderFactory`, `ProviderDescriptor`, `ProviderEvent`
- [x] `ProviderRegistry` in runtime (inventory collection, technology grouping, instance creation)
- [x] Event pipeline: dispatcher, shutdown cleanup, `subscribe_events`, `register_event_sink`
- [x] Per-provider snapshots (polling vs. event-driven refresh)
- [x] Inventory macros (`register_provider!`, `register_platform_module!`) + tests
- [x] Factory lifecycle documented (no additional services passed)
- [x] Provider checklist via contract test suite
- [x] `ProviderDescriptor` extended with `event_capabilities` bitset

### 10.5 CLI `list-providers`

- [x] Minimal runtime + mock platform/provider path
- [x] `platynui-platform-mock` groundwork
- [x] `platynui-provider-mock` via factory handle
- [x] CLI command with text/JSON output
- [x] Tests against mock setup

### 10.6 CLI `info`

- [x] `DesktopInfoProvider` trait in core
- [x] Mock desktop info data
- [x] Runtime builds desktop document node from provider
- [x] CLI command (text/JSON)
- [x] Tests (multi-monitor, OS variants)

### 10.7 CLI `query`

- [x] Scriptable mock tree (`StaticMockTree`) with deterministic RuntimeIds
- [x] `evaluate(node, xpath, options)` API
- [x] CLI command with format options and filters
- [x] Tests against mock tree
- [x] Reference tree in `crates/provider-mock/assets/mock_tree.xml`
- [x] Tree definition loaded from XML
- [x] Text output in XML-like format, JSON with full attribute lists
- [x] Provider dual-view (flat + grouped) with stable doc-order keys

### 10.8 CLI `watch`

- [x] Event pipeline wired to CLI
- [x] Mock provider event simulation
- [x] Runtime respects `event_capabilities`
- [x] CLI command with streaming output, `--expression`, `--limit`
- [ ] Watch filters: `--namespace`, `--pattern`, `--runtime-id`
- [x] Tests for simulated event sequences

### 10.9 XDM Cache (Lazy Revalidation)

- [x] `XdmCache` type implemented (`Rc<RefCell<...>>`, `Clone`, `!Send`)
- [x] `EvaluateOptions` with `with_cache()`/`without_cache()`
- [x] Lazy revalidation: `is_valid()`, `prepare_for_evaluation()`, transparent rebuild
- [x] Convenience methods: `evaluate_cached()`, `evaluate_iter_cached()`, etc.
- [x] CLI `watch` uses cache for repeated evaluations
- [x] Python bindings: thread-local cache per `PyRuntime`, `clear_cache()`
- [x] Benchmark: ~40% faster for repeated queries
- [ ] Event-driven cache invalidation (Option B) — see §3.1

### 10.10 CLI `highlight`

- [x] `HighlightProvider` finalized
- [x] Mock highlight logging
- [x] CLI command with XPath, `--duration-ms`, `--clear`
- [x] Tests

### 10.11 CLI `screenshot`

- [x] `ScreenshotProvider` trait defined
- [x] Mock screenshot (deterministic RGBA gradient)
- [x] CLI command with `--rect`, path argument, PNG encoding
- [x] Tests

### 10.12 CLI `focus`

- [x] `FocusablePattern` in mock tree (dynamic `IsFocused`, events)
- [x] `Runtime::focus()` with differentiated errors
- [x] CLI command with success/skip lists
- [x] Tests

### 10.13 Runtime Pattern Integration (Mock)

- [x] Mock provider: `FocusableAction` + dynamic `IsFocused`
- [x] PatternRegistry/lookup tests
- [x] `PatternRegistry::register_lazy` with on-demand probing

### 10.14 CLI `window`

- [x] `WindowSurface` pattern in mock (all actions, dynamic attributes)
- [x] CLI command: actions + `--list`
- [x] Tests (listing, action sequences, error paths)
- [x] XPath normalization split: `EnsureDistinct`/`EnsureOrder`

### 10.15 CLI `pointer`

- [x] `PointerDevice` trait (all operations, double-click metadata)
- [x] Motion engine (linear/bezier/overshoot/jitter, configurable delays)
- [x] Mock pointer with logging hooks
- [x] CLI parsers (coordinates, scroll deltas, buttons)
- [x] CLI command with all subcommands and options
- [x] Tests (motion engine, CLI integration)

### 10.16 Keyboard — Trait & Settings

- [x] `KeyboardDevice` trait (`key_to_code`, `send_key_event`, `known_key_names`)
- [x] Key naming conventions documented
- [x] `KeyboardEvent` struct
- [x] `KeyboardSettings` + `KeyboardOverrides`
- [x] Documentation updated

### 10.17 Keyboard — Sequence Parser & Runtime API

- [x] `KeyboardSequence` parser (strings, shortcuts, escapes, iterators)
- [x] Event resolution (strict `key_to_code` matching)
- [x] Runtime API (`keyboard_type`, `keyboard_press`, `keyboard_release`)
- [x] Unit tests
- [x] Documentation

### 10.18 Keyboard — Mock & CLI

- [x] Mock keyboard with logging
- [x] Mock text handling (emojis, IME strings)
- [x] CLI commands (`type`, `press`, `release`, `list`)
- [x] Tests
- [x] README/CLI help updated

### 10.19 Mock Fallback & Build Assignment

- [x] `mock-provider` feature documented
- [x] Verified: real platform modules only via `cfg(target_os = ...)`

### 10.20 Windows Platform — Devices & UiTree

#### Pointer
- [x] Win32 SendInput pipeline (move/click/drag/scroll, double-click metrics)
- [x] Runtime registry integration, PointerOverrides/Profile validation
- [x] Per-Monitor-V2 DPI awareness
- [x] Tests (negative coordinates, DPI documentation)

#### Highlight
- [x] Overlay lifecycle (create/update/clear, Z-order, colors)
- [x] Runtime binding + fallback behavior

#### Screenshot
- [x] GDI capture (BitBlt), cropping, format conversion
- [x] Runtime wiring, parameter documentation

#### Platform Init
- [x] `PlatformModule::initialize()` sets DPI awareness
- [x] Init ordering test (platform before providers)

#### Desktop Provider
- [x] Windows DesktopInfoProvider (monitors, bounds, RuntimeId)
- [x] Monitor enumeration, friendly names, OS version, DPI scale
- [x] Smoke tests

#### UIA Provider
- [x] COM init (MTA), thread-local singletons
- [x] Tree traversal via Raw View TreeWalker
- [x] Lazy iterators
- [x] Role/namespace mapping, RuntimeId
- [x] Patterns: Focusable (SetFocus), WindowSurface (WindowPattern/TransformPattern)
- [x] Grouped view (Application nodes by PID)
- [x] Application attributes (ProcessId, Name, ExecutablePath, etc.)
- [x] WindowSurface status attributes
- [x] `accepts_user_input()` (heuristic + WaitForInputIdle)
- [x] Virtualized elements (best-effort Realize)
- [x] Native UIA properties (programmatic name scan, type conversion, sentinel filtering)
- [ ] Tests: Windows smoke for iterator order, app attributes, native properties
- [ ] Tests: structure/attribute coverage, pattern list, desktop top-level

#### Keyboard
- [x] Key-code resolution and event injection via SendInput
- [x] VK name map (all VK_* constants without prefix)
- [x] Character path (VkKeyScanW, CapsLock handling, Unicode fallback)
- [x] AltGr handling (VK_RMENU injection)
- [x] Extended keys flagging
- [x] Left/right modifier aliases
- [x] Symbol aliases (PLUS, MINUS, LESS, GREATER)
- [x] `known_key_names()` implementation
- [ ] Error mapping refinement (Win32 LastError)
- [ ] Tests: AltGr on DE layout, CapsLock, extended keys, OEM/ABNT/DBE keys
- [ ] Evaluate `VkKeyScanExW` with thread layout (HKL) for multi-layout scenarios

#### Focus & WindowSurface via UIA
- [x] Focus control (SetFocus) and WindowSurface actions
- [x] Error mapping in FocusableAction/WindowSurface
- [x] WaitForInputIdle integration in `accepts_user_input()`
- [ ] Extended error handling: foreground locks, UAC, non-responsive apps
- [ ] Integration tests (WPF, WinForms, Win32, UWP)
- [ ] Documentation: flow diagrams, troubleshooting

#### ScrollIntoView via UIA
- [ ] `scroll_into_view()` using `ScrollItemPattern::ScrollIntoView()` + `VirtualizedItemPattern::Realize()`
- [ ] Dynamic container search via TreeWalker
- [ ] Error handling for UIA-specific failure states
- [ ] CLI pointer integration
- [ ] Tests

#### Tests & Mock Alignment
- [ ] Shared tests (provider vs. mock) for bounds, ActivationPoint, focus, WindowSurface
- [ ] UIA API deviation documentation and regression playbooks
- [ ] Test infrastructure (Windows CI job)

### 10.21 CLI `window` — Windows Integration

- [x] Window listing with status/capabilities, all actions, bring_to_front + wait
- [x] `bring_to_front_and_wait` runtime/Python extension
- [ ] `bring_to_front` + `ensure_window_accessible()` integration
- [x] Tests (mock coverage, action sequences)

### 10.22 Linux X11 — Devices & UiTree

- [x] DesktopInfoProvider (XRandR, root fallback)
- [x] Pointer via XTest
- [x] Keyboard via XTest + `GetKeyboardMapping` (named keys, character input, CapsLock handling, dynamic remap, control chars)
- [x] Screenshot via XGetImage
- [x] Highlight via override-redirect segments
- [x] `PlatformModule::initialize()` (eager connect, XTEST/RANDR checks)
- [x] Window management via EWMH/NetWM
- [ ] Focus helper for AT-SPI2
- [ ] Tests: desktop bounds, ActivationPoint, visibility under X11
- [x] AT-SPI2 provider: D-Bus integration, RuntimeId, role mapping, streaming attributes
- [x] AT-SPI2: component-gated attributes, Focusable pattern
- [x] AT-SPI2: native interface attributes
- [ ] AT-SPI2: tree structure verification
- [ ] AT-SPI2 supplementary tests
- [ ] Wayland mediation crate planning

### 10.23 Tools

- [x] CLI `watch` with text/JSON and query follow-up
- [x] CLI `--json` for `query`
- [x] CLI pointer commands with element descriptions
- [ ] CLI `dump-node`
- [ ] Script integration / CLI ergonomics
- [ ] Inspector: tree view, property panel, XPath editor, element picker, highlight
- [ ] Example workflows documented

#### CLI `snapshot`
- [x] Specification created
- [x] CLI scaffold with args parser
- [x] Streaming XML writer (quick-xml)
- [x] Attribute filters (`--attrs`, `--include`/`--exclude`)
- [x] Alias attributes (default generate, `--exclude-derived`)
- [x] Depth limiting (`--max-depth`)
- [x] Multi-root handling (wrapper `<snapshot>`, `--split`)
- [x] Pretty mode
- [x] Tests (golden comparisons, error cases)
- [x] Documentation

### 10.24 Quality & Processes

- [x] CI pipeline: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run`, `ruff`, `mypy`, wheel builds
- [ ] Contract tests for providers & devices
- [ ] Documentation maintenance
- [ ] Release/versioning strategy

### 10.25 Backlog & Explorations

- [x] Context node resolver (RuntimeId-based re-resolution)
- [ ] Out-of-process provider support (see §3.4)
- [ ] macOS (AX) provider (see §4.3)
- [ ] macOS platform module (see §4.3)
- [ ] Optional provider runtime services interface
- [ ] Persistent XPath caching & snapshot layer
- [ ] Wayland support (see §4.4)
- [ ] Additional patterns (table navigation, drag & drop)
- [ ] Extended input devices (gamepad, stylus, touch)
- [ ] Community guides, example providers

### 10.26 UiNode `Id` — Implementation Steps

**Core:**
- [x] Attribute defined: `control:Id` as optional string, constants in `attribute_names`
- [x] `UiNode` trait extended with `fn id(&self) -> Option<String>` (default `None`)
- [x] Documentation updated (architecture/patterns/checklist)

**Runtime/XPath:**
- [x] `@control:Id` atomizes as `xs:string` (verified)

**Provider:**
- [x] Windows/UIA: `AutomationId` → `control:Id`
- [x] Windows/ApplicationNode: `id()` returns process name
- [ ] AT-SPI2: map `accessible_id` if available
- [ ] macOS/AX: map `AXIdentifier` if available
- [ ] Application nodes: platform-appropriate stable identifier
- [ ] Windows option: evaluate AUMID as application Id

**Tests:**
- [ ] Core contract tests for `Id`
- [ ] Provider smoke tests
- [ ] CLI/Python example queries documented

### 10.27 Distribution & Packaging

- [x] CLI as Python wheel (`packages/cli`, maturin bindings = bin)
- [x] Native Python bindings (`packages/native`, PyO3/maturin)
- [x] Local development workflow documented
- [x] CI wheel builds for Linux/macOS/Windows

### 10.28 Robot BareMetal (Interim)

- [x] Robot Framework library `PlatynUI.BareMetal` with keywords
- [ ] Additional acceptance test suites under `tests/BareMetal/`

### 10.29 WindowManager — Platform-Native Window Control

**Phase 1 — Core Trait:**
- [x] `WindowManager` trait in `platynui-core::platform`
- [x] `WindowId` as opaque `u64` type
- [x] `register_window_manager!` macro + `window_managers()` iterator
- [x] Mock implementation in `platynui-platform-mock`

**Phase 2 — X11/EWMH:** ✅
- [x] Migrate `ewmh.rs` from `provider-atspi` to `platform-linux-x11`
- [x] `resolve_window()`: PID + `_NET_WM_NAME` + `_NET_CLIENT_LIST` matching
- [x] EWMH support check in `PlatformModule::initialize()`
- [ ] `ensure_window_accessible()`: `_NET_WM_DESKTOP` + `_NET_CURRENT_DESKTOP`
- [ ] Tests

**Phase 3 — Windows/Win32:**
- [x] `WindowManager` implementation
- [x] `resolve_window()`: `native:NativeWindowHandle` → HWND
- [x] All window operations (bounds, activate, close, minimize, maximize, restore, move, resize)
- [ ] `ensure_window_accessible()` via `IVirtualDesktopManager`
- [ ] Tests

**Phase 4 — Provider Migration:** ✅
- [x] `provider-atspi`: remove `ewmh.rs` and `x11rb` dependency
- [x] Replace direct window calls with `WindowManager` trait calls
- [ ] `provider-windows-uia`: optional `WindowManager` fallback
- [x] Existing tests green

**Phase 5 — Wayland (Future):**
- [ ] `platform-linux-wayland`: `WindowManager` via `wlr-foreign-toplevel-management` (Sway/Hyprland)
- [ ] `resolve_window()`: App-ID + PID matching
- [ ] Evaluate compositor-specific backends (KDE D-Bus, COSMIC workspace protocol)
- [ ] Mediation crate `platynui-platform-linux` for X11/Wayland runtime selection (see §4.4)

**Phase 6 — macOS (Future):**
- [ ] `platform-macos`: `WindowManager` via AppKit/CoreGraphics

### 10.30 Inspector (egui)

**egui Implementation:** ✅
- [x] MVVM architecture (Model/ViewModel/View separation)
- [x] Tree panel: expand/collapse, keyboard navigation (Up/Down/Left/Right/Home/End/PageUp/PageDown)
- [x] Properties panel: sortable columns (Name/Value/Type), read-only selectable text, context menu (Copy Name/Value/Type/Row)
- [x] XPath search bar with results panel, click-to-reveal in tree
- [x] UiNodeData with Mutex-based caching (id, label, children, has_children)
- [x] Element highlighting on selection (1.5s via platform highlight provider)
- [x] Role icons, invalid-node strikethrough, selection/focus indicators
- [x] Context menu on tree rows (Refresh / Refresh Subtree)
- [x] Always On Top toggle
- [x] Tracing integration (--log-level CLI flag)
- [x] No build.rs / no code generation
- [x] Streaming XPath search: non-blocking background thread, mpsc streaming, cancel flag, spinner, Stop button (see §5.1.1)

**Remaining:**
- [x] TreeView widget component: generic, self-contained egui widget with built-in keyboard nav + virtual scrolling (see §5.1.2)
- [ ] Element Picker (click-to-identify)
- [ ] Export (copy XPath for selected node, subtree as XML)
- [ ] Performance measurement with large trees (≥2k visible nodes)
- [ ] Async child loading (non-blocking tree expansion)
- [ ] Filter in properties panel
