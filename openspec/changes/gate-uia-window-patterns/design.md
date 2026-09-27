# Design

## Context

See proposal.md for the motivation. Facts were verified in the working tree at `999ce95`. The measurements come from a scratch crate outside the repository that shows a window like the test window of the UI Automation tests in a child process, without activating it. *Inferred* marks conclusions that were not run.

**UI Automation today.**

- `supported_patterns` advertises the eight window patterns only `if self.has_window_surface()` (`crates/provider-windows-uia/src/node.rs:503-524`). The window surface is a WindowPattern or a TransformPattern, read once per node with two cross-process calls that do not short-circuit (`:251-260`).
- `pattern_by_name` returns the eight window instances with no gate (`:560-568`, built at `:673-735`), while Focusable is gated on both sides (`:535`).
- `ElemSend::activate` (`:588-636`):
  - calls `ShowWindow(SW_RESTORE)` only if the element's *own* handle is iconic;
  - calls UIA `SetFocus` on the element;
  - polls up to one second for `GetForegroundWindow()` to equal the element's own handle. That can never succeed for a child-window control.
- History:
  - ungated since the first UIA commit `2d575d7`;
  - element `SetFocus` since `6bac989`;
  - the poll since `3a805c7`, added because tests read `@IsActive` right after activation;
  - `SW_RESTORE` since `1312085` (`window-activation-state`).
- AccessKit refuses `SetFocus` on a disabled node with `UIA_E_ELEMENTNOTENABLED` (`accesskit_windows` 0.32.1 `src/node.rs:902-917`, `:1066-1068`).

**The window manager is already handed to UIA, and UIA drops it.**

- `UiTreeProvider::set_window_manager` exists with a no-op default (`crates/core/src/provider/tree_provider.rs:63-68`). The runtime calls it on every provider once it has a platform (`crates/runtime/src/runtime/mod.rs:207-215`).
- UIA does not override it (`crates/provider-windows-uia/src/provider.rs:442-551`).
- JAB (`crates/provider-java-jab/src/node.rs:1264-1285`), the agent (`crates/provider-java/src/agent/node.rs:703-725`) and AT-SPI (`crates/provider-atspi/src/extents.rs:32-53`) keep it and activate through it.
- `dev-docs/architecture.md:676-683` already draws UIA as a user of the window manager.
- The Win32 window manager's `activate` (`crates/platform-windows/src/window_manager.rs:222-270`):
  - restores an iconic window;
  - attaches the thread input of the foreground thread and the target thread;
  - calls `BringWindowToTop` and `SetForegroundWindow`;
  - logs `tracing::warn!` on every refusal (`:263-268`).
- `resolve_window` (`:157-178`) takes the node's `native:NativeWindowHandle` as it is. When the handle cannot be read, it falls back to the first visible window of the process (`:108-144`).
- UIA already builds `WindowId::new(hwnd)` from a handle itself, for the Java classifier (`node.rs:306`).

**Consumers.**

- `Runtime::top_level_window_for` (`crates/runtime/src/runtime/window.rs:41-58`) returns the first ancestor-or-self with `pattern::<ActivatableAction>()`, which goes through `pattern_by_name`, so on UIA it returns the element itself.
- `UiNodeExt::top_level_or_self` checks `supported_patterns()` instead (`crates/core/src/ui/node.rs:167-181`); the two disagree today.
- Native `get_pattern` checks the advertisement first (`packages/native/src/runtime.rs:265-272`), so BareMetal Activate Window already fails on a non-window UIA element.
- BareMetal's implicit activation (`src/PlatynUI/BareMetal/__init__.py:1770-1800`) swallows failures at debug. The explicit Bring To Front propagates them (`:2363-2382`).
- The CLI uses `pattern::<ActivatableAction>()` as its "is a window" marker (`crates/cli/src/commands/window.rs:170-176`).

**The test window.**

- The window of `TestWindow` (`crates/provider-windows-uia/src/node.rs` tests, added by `xdm-snapshot-release`):
  - is of the `STATIC` class, with a WindowPattern and a TransformPattern (measured);
  - has role `Text`, not `Window`;
  - does not advertise Focusable.
- Its buttons have handles of their own, whose root is the window, and neither pattern.
- The title bar and its system menu have no handle and neither pattern.
- Their names are localized (measured: "Systemmenü", "Minimieren").
- Reading the two availability properties with the traversal cache request (`crates/provider-windows-uia/src/com.rs:140-157`) had no measurable cost on Win32 controls. AccessKit was not measured.

**Contract checks.**

- The testkit's `verify_node` reads only `supported_patterns()` (`crates/core/src/ui/contract/testkit.rs:195-240`).
- `validate_control_or_item` claims "all advertised runtime patterns provide concrete implementations" (`crates/core/src/ui/contract.rs:16-17`), but checks only the namespace and duplicates. Its test `runtime_pattern_without_instance_is_allowed` (`:220-223`) pins that behavior.
- The Java live tests check advertised ⇒ instance inline for every non-marker pattern (`crates/provider-java/tests/live_fixture.rs:319-333`, `:1057-1068`).
- The mock advertises attribute-only patterns without instances by design (`crates/provider-mock/src/tree.rs:571-573`).

**Specs.** `window-activation` exists only as a delta of the active change `window-activation-state`. OpenSpec rejects MODIFIED on a capability without a main spec, so this change only adds requirements. Its delta repeats that capability's Purpose word for word, so that the main spec is the same whichever change is archived first.

## Goals / Non-Goals

**Goals:**

- UIA hands out a window pattern exactly where it advertises one, and every provider is checked for that.
- UIA activation raises the root window through the runtime's window manager, without moving the focus inside it and independent of the element's enabled or focusable state.
- No wait of up to a second, no warning flood, and no noticeable cost from walking the ancestors.

**Non-Goals:**

- Routing Minimize, Maximize, Restore, Close, Move and Resize through the window manager. They keep using UIA's patterns, which also work on nested window surfaces.
- The finer gate: Minimize, Maximize, Restore and Close only with a WindowPattern, Move and Resize only with a TransformPattern. Today an element with only a TransformPattern advertises Minimizable, and the action then fails. That is a follow-up.
- Activating a nested window (an MDI child, a UWP `CoreWindow`) on its own. Only its root window is raised (decision 5).
- Changing the mock provider. Its activation moves the mock's focus to the window node, which is the mock's model of an application. The new focus requirement is about what PlatynUI requests, which the mock already meets.
- Changing how the Win32 window manager resolves windows.

## Decisions

### 1. The window manager is injected into UIA through the existing seam

`WindowsUiaProvider` overrides `set_window_manager` with a set-once slot, like `set_java_classifier` (`provider.rs:407-452`). It passes `Option<Arc<dyn WindowManager>>` to every node it creates:

- the top-level windows of `ElementAndAppIter`;
- `ApplicationNode` and `AppWindowIter`;
- the hit-test chain of `element_at_point` (`attach_ancestor_chain`);
- `ElementChildrenIter`, which hands it on from its parent.

That is how AT-SPI, JAB and the agent carry it. The manager belongs to the runtime, so a runtime with the mock platform gets the mock manager.

*Alternatives rejected:*

- **UIA depends on `platynui-platform-windows`.** No provider depends on a platform crate; it would break the layering of `dev-docs/architecture.md` §8.5.
- **A new shared crate for the foreground workaround, or a copy of it in UIA.** Two copies would drift, and UIA would stay the one provider that bypasses the runtime's window manager.

This reverses `window-activation-state` D4, which rejected the injection only as too large for that fix. Here it is the purpose.

### 2. UIA resolves the root window itself and hands the window manager a WindowId

- For a window element, the provider reads the element's native window handle. If that is 0, it walks the raw-view UIA parents to the nearest element with a handle (bounded depth).
- It takes `GetAncestor(handle, GA_ROOT)` and calls `window_manager.activate(WindowId::new(root))`.
- The Win32 window manager interprets a WindowId as a window handle. UIA and the Win32 manager already rely on that for the Java classifier (`node.rs:306`). This is written down at the new code and in `dev-docs/platform-windows.md`.

*Alternatives rejected:*

- **The provider calls `resolve_window(node)`, and the Win32 manager normalizes to the root there.** It would keep the WindowId opaque. But it changes `resolve_window` for JAB and the agent too. And when the second read of the handle fails (an element that went stale in between), it falls back to the first visible window of the process (`window_manager.rs:108-144`) and activates a window the user never named. Resolving in UIA reads the handle once and never guesses.

### 3. `pattern_by_name` is gated like `supported_patterns`

- The window branch (`node.rs:560-568`) returns an instance only if `has_window_surface()`; one helper serves both methods.
- `has_window_surface` short-circuits: it asks for the TransformPattern only when the WindowPattern is missing.
- The traversal cache request reads `IsWindowPatternAvailable` and `IsTransformPatternAvailable`, and `populate_cached_properties` fills the value in. That makes the walk of `top_level_window_for` over nodes listed by `ElementChildrenIter`, which covers every query result's ancestors, free of extra cross-process calls.

*Alternatives rejected:*

- **Fixing only `top_level_window_for`, so that it checks `supported_patterns`.** UIA would stay dishonest to every other caller of `pattern_by_name`: the Python `ancestor_pattern`, the CLI marker, and third-party code.

### 4. The route and its precedence

1. A native handle was found (own or an ancestor's) and a window manager is injected: activate the root through the window manager (decision 2). Then wait until `window_manager.is_active(id)` or for at most one second, as `3a805c7` did, because existing lane tests read `@IsActive` once, right after activation (`tests/acceptance/egui/auto_activate.robot:43-58`). This wait targets the root, which the window manager does report active.
2. A handle was found but no window manager is injected: fail with a `PatternError` that names the missing window manager, as JAB, the agent and AT-SPI do.
3. No handle anywhere in the chain: UIA `SetFocus` on the element, today's path. That happens only for a window surface outside every window, which no fixture of the repository has.

The decision is a pure function of (handle found, manager present), unit-tested like `advertise_focusable` (`node.rs:89`).

*Alternatives rejected:*

- **Falling back to `SetFocus` when no window manager is injected.** It would bring back the focus side effects silently in providers-only runtimes, and differ from every other provider.
- **Dropping the wait.** Lane tests depend on it.

### 5. Nested window surfaces raise their root only

An MDI child, or a UWP `CoreWindow` inside its frame, has a handle whose root is its frame. Activation raises the frame. The nested surface is neither restored nor activated on its own. Its `@IsActive` compares the foreground window with its own handle (`node.rs:1647-1671`), so it reads False after the activation. The spec rule "after activation the window SHALL be the active window" therefore holds for the root, not the nested surface. That limitation is written into `dev-docs/platform-windows.md`. No fixture of the repository has nested windows, and the ones Windows brings are not used for tests.

### 6. The foreground-refusal warning is logged once per episode

The Win32 window manager logs a warning on every refused `SetForegroundWindow` (`window_manager.rs:263-268`). UIA activation now reaches it before every pointer, keyboard, highlight and screenshot action. It follows the once-per-episode rule of `dev-docs/logging.md`:

- a warning the first time for a window;
- debug while the refusals continue;
- debug when an activation succeeds again.

This uses `platynui_core::diagnostics::Transitions`. The Windows lane stays free of warnings.

### 7. A testkit check proves the contract for every provider

- `verify_pattern_instances(node) -> Vec<ContractIssue>` checks every action pattern in both directions and the TextEditable marker, and exempts attribute-only patterns.
- It never runs an action.
- The action patterns come from one list next to `declare_action_pattern!` (`crates/core/src/ui/pattern.rs:340-344`), so a new action pattern is checked without touching the testkit.
- New `ContractIssue` variants: `PatternWithoutInstance`, `InstanceWithoutAdvertisement`, `MarkerWithInstance`.
- Where it runs:
  - the mock tree (CI);
  - the test window and its subtree (UIA, `just test` on Windows);
  - the Java live tests. There it is added *next to* their inline loops, because those also require an instance for every advertised non-marker pattern, which is stricter in that direction.
- `validate_control_or_item` keeps its semantics, which its test pins. Only its documentation is corrected, and it points to the new check.

### 8. Tests are placed by what they touch

- Read-only UIA tests (gate, subtree, ancestor lookup, root determination, the no-manager error) run in plain `just test` on a Windows host, on the test window of `xdm-snapshot-release`.
- The test that activates the window with the real Win32 window manager is ignored. The manager comes from `platform_factories()`, as in `crates/provider-java/tests/live_fixture.rs:231-240`, with `platynui-platform-windows` as a dev-dependency. `just test-acceptance-windows` runs it once `-p platynui-provider-windows-uia` is added to its ignored-only run (`justfile:385`).
- The focus and disabled-button scenarios are egui acceptance tests without a platform tag, so every lane runs them.
- No test uses the taskbar, the shell or applications that Windows ships.

### 9. Qt and QML popups are measured before gating

Today the implicit activation before a click on a Qt or QML menu item is a UIA `SetFocus` on the item. After gating, it depends on the popup: if the popup window has a WindowPattern or TransformPattern, activation calls `SetForegroundWindow` on the popup's root; otherwise it finds no Activatable and proceeds. The first step of the implementation reads `@SupportedPatterns` of a context-menu popup of the Qt test app and a popup of the QML test app on the Windows lane (a real build), and records the result in the tasks. If activating a popup root closes the popup, a `WS_EX_NOACTIVATE`/popup check in the root determination is added before gating, and the plan is updated.

## Risks / Trade-offs

- **[Foreground refusals now show up more often]** → Once per episode (decision 6). The lane's zero-warning check makes a regression visible.
- **[`AttachThreadInput` now runs before every UIA pointer action]** → The path is not new (JAB and the agent use it) but is used far more often. A hung target thread can stall it. The lane measures the suites' time before and after; a stall becomes a follow-up in the window manager, not a reason to keep `SetFocus`.
- **[Qt/QML popups]** → Measured first (decision 9).
- **[Elements with no window above them lose Bring To Front]** → Correct by the spec ("element without an activatable window"). Listed in the release notes. The implicit activation still proceeds, as before.
- **[Nested window surfaces]** → Root only (decision 5). This is untested for lack of a fixture, and written down.
- **[AccessKit dialog nodes]** → A dialog node without a handle inside an egui window resolves through its ancestors to the host window (decision 2), so it no longer takes the focus route. `accesskit_windows` implements a WindowPattern for dialog nodes (`src/node.rs:698-700`), which no repository fixture has.
- **[Walking the ancestors costs calls]** → The cache request and the short-circuit (decision 3); measured without noticeable cost on Win32 controls.
- **[UIA tests run only on Windows hosts]** → The Windows job of CI only lints. The tasks require a Windows `just test` run, and the lane.
- **[Coordination]** → `window-activation-state` changed the same `activate` code and has open tasks. Its Windows lane should run first, so that its SW_RESTORE scenario is recorded on the old path before this change replaces it. `snapshot-validity` and `application-process-attributes` touch `ApplicationNode` and should land one after another with this change.

## Migration Plan

- **Behavioral.** No public API changes. The core testkit gains a check and three `ContractIssue` variants, which are additions for the release notes.
- **Native rebuild:** yes.
- **Sequence:**
  1. The popup measurement.
  2. The tests: RF acceptance and Rust, red where the spec says so.
  3. The testkit check.
  4. The window manager's warning.
  5. The UIA injection, gate, route and cache.
  6. The docs.
  7. The lanes.
- **Rollback:** revert. The gate and the route belong together: gating without the window-manager route would lose activation on every element whose ancestors lack a window surface. Reverting only the route would bring back `SetFocus` on the root window.

## Open Questions

- Should the bounded wait for the foreground change move into the Win32 window manager's `activate`, so that JAB and the agent get it too? It is not needed for this change, which keeps the wait in UIA.
