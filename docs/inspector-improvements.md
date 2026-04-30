# Inspector Improvements Design

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document describes proposed usability and architecture improvements for the PlatynUI Inspector. It complements the current implementation overview in `docs/inspector.md` and is intended to help choose the next implementation slices.

## 0. Current Status Snapshot (2026-04-24)

- `5.1 Results Panel Workflow and Actions`: **Implemented** (explicit reveal actions, highlight action, context menu copy flows, no auto-reveal on arrow navigation).
- `5.2 Attribute Filter and Pinned Attributes`: **Partially Implemented** (text filter, pinning, grouped/ungrouped view, collapsible groups are in place; category chips are still optional/future).
- `5.3 Breadcrumb and Selected-Node Path Bar`: **Deferred** (still valid, not implemented).
- `5.4 Tree Refresh and Loading Feedback`: **Not Planned for Now** (still valid).
- `5.5 Diagnostic Status Bar`: **Revisited / Partially Implemented** (status bar now carries meaningful async status text + spinner; broad diagnostics surface is still intentionally limited).
- `5.6 Command Palette and Shortcut Help`: **Not Started** (still valid).
- `5.7 Freeze and Snapshot Mode`: **Not Started** (still valid).
- `5.8 Documentation and Regression Coverage`: **Partially Implemented** (interaction behavior evolved, but docs/test coverage still need follow-up).
- `5.9 Async Execution Hardening and Performance`: **Partially Implemented / Next** (core `egui-async` task migration, bounded search progress draining, search restart/cancel, typed status, selection/reveal epochs, highlight epochs, delayed node validity checks, and a configurable broad-search result limit are done; startup/shutdown policy, deeper instrumentation, and tests remain).

The sections below keep the original design intent but now include explicit status markers and updated next slices.

## 1. Goals

- Improve day-to-day inspection speed for large and dynamic UI trees.
- Reduce accidental or surprising UI behavior.
- Make existing commands and shortcuts easier to discover.
- Keep the current MVVM split intact.
- Keep provider I/O and expensive tree work off the UI thread.

## 2. Non-Goals

- Replace egui with another UI toolkit.
- Redesign the Inspector visual language from scratch.
- Introduce a new runtime/provider abstraction unless needed for a specific feature.
- Implement all improvements in one work package.

## 3. Guiding Principles

- Commands should have a single source of truth and be reusable from menu bar, toolbar, context menus, shortcuts, and future command surfaces.
- Navigation and activation should be separate where accidental activation is costly.
- Live inspection should remain responsive, but mode changes must be explicit.
- Expensive operations should provide visible feedback.
- Read-only inspection controls should support expected desktop text handling and copy flows.

## 4. Current Pain Points

- The results panel is fast, but it can feel too eager because tree reveal is tightly coupled to keyboard focus movement.
- The attributes table works for small nodes but becomes hard to scan for large nodes with many attributes.
- The current UI does not communicate enough context about where the selected node sits in the tree.
- Command discoverability still depends too much on users exploring menus manually.
- There is no stable inspection mode for fast-changing UIs.
- Documentation and regression coverage lag behind recent Inspector interaction work.

## 5. Detailed Proposals

### 5.1 Results Panel Workflow and Actions

**Status:** Implemented

#### Problem

The current results panel couples keyboard focus movement and tree reveal too tightly. This is efficient when the user wants a quick jump, but it can also produce a distracting "tree keeps moving" effect while browsing results. The panel also exposes only a narrow action surface.

#### Goals

- Separate lightweight result browsing from expensive reveal behavior.
- Add richer result-specific actions without forcing a reveal first.
- Make keyboard behavior predictable.

#### Proposed UX

- Arrow keys move the focused result row only.
- `Enter` reveals the focused result in the tree.
- Double-click reveals the clicked result immediately.
- `Space` or `Ctrl/Cmd+H` highlights a node result without changing tree focus.
- Right-click on a result opens a context menu with:
  - `Reveal in Tree`
  - `Highlight Result`
  - `Copy Label`
  - `Copy Runtime ID` for node-backed results
  - `Copy Attribute Value` for attribute results
  - `Copy Full Result`
- Optional later toggle: `Auto-Reveal Focused Result` for users who prefer the current behavior.

#### Technical Design

- Extend `ResultAction` in `apps/inspector/src/view/results_panel.rs` beyond `Reveal(usize)` to include dedicated actions such as `Highlight(usize)`, `CopyLabel(usize)`, and `CopyResult(usize)`.
- Route these actions through the existing command dispatcher in `apps/inspector/src/lib.rs`.
- Keep result row rendering in the view layer; ViewModel stays responsible for reveal/highlight logic.
- Add a small `ResultCommand` helper or fold result actions into the existing `AppCommand` model if command reuse becomes common.

#### Risks and Tradeoffs

- Removing auto-reveal entirely may feel like a regression to users who prefer the current fast-jump workflow.
- A toggle adds complexity, so it should come only if needed.

#### Recommended Slice

1. Add result context menu and explicit `Enter`/double-click reveal.
2. Stop auto-reveal on arrow-key movement.
3. Evaluate whether an optional auto-reveal setting is still needed.

**Status review:** The recommended slice has effectively landed. Only the optional auto-reveal toggle remains open if user feedback requests it.

### 5.2 Attribute Filter and Pinned Attributes

**Status:** Partially Implemented

#### Problem

The attributes pane is already sortable, filterable, copy-friendly, supports per-attribute pinning, can switch between grouped and ungrouped namespace views, and supports collapsible namespace groups, but long attribute lists are still noisy. For large or provider-heavy nodes, the user spends too much time visually scanning for the same small set of fields.

#### Goals

- Make common attributes immediately visible.
- Reduce scan time for large attribute sets.
- Preserve the current flat-table simplicity.

#### Proposed UX

- Keep the current text filter, pinning model, grouped/ungrouped namespace toggle, and collapsible namespace groups as the baseline interaction.
- Add optional quick filters as chips or buttons:
  - `Identity`
  - `Geometry`
  - `State`
  - `Patterns`
  - `Native`

#### Technical Design

- Use `namespace:name` as the canonical attribute key.
- Keep filtering and sorting in `apps/inspector/src/view/attributes.rs`, using ViewModel state as input.
- Render pinned attributes in a separate block above the normal sorted rows, then render remaining rows below.
- Do not mutate the underlying attribute data; derive the visible row order at render time.

#### Risks and Tradeoffs

- A large number of pinned rows can recreate the same clutter problem in a different place.
- Category filters may need stable attribute classification rules per namespace.

#### Recommended Slice

1. Keep the current text filter and pinning behavior stable.
2. Add category chips only after real usage shows a need.

**Status review:** Baseline behavior is implemented and stable. Category chips remain a future, usage-driven enhancement.

### 5.3 Breadcrumb and Selected-Node Path Bar (Deferred)

**Status:** Deferred (still valid)

#### Problem

Once a node is selected, the UI shows its attributes and highlights it, but it does not provide a compact answer to "where is this node in the hierarchy?"

This is still interesting, but it is not a current priority. Revisit it only if real user feedback shows that orientation inside deep trees is a recurring pain point.

#### Goals

- Improve orientation inside deep trees.
- Make ancestor navigation cheaper.
- Provide a copyable path representation.

#### Proposed UX

- Add a breadcrumb bar above the attributes pane or above the tree pane.
- Each segment represents one ancestor, ending with the selected node.
- Clicking a segment selects that ancestor in the tree.
- Right-click on a breadcrumb segment offers:
  - `Select`
  - `Copy Label`
  - `Copy Runtime ID`
  - `Copy Full Path`
- If the tree is partially loaded, the breadcrumb should still show the known selection path, even if some ancestors are not currently visible.

#### Technical Design

- Add a lightweight `SelectedPathItem` model in `InspectorViewModel` with fields such as `label`, `runtime_id`, and `tree_index: Option<usize>`.
- Build the path when selection changes, using the selected `UiNodeData` parent chain rather than depending on currently visible rows.
- Resolve visible `tree_index` opportunistically so breadcrumb clicks can reuse `select_node` when possible.

#### Risks and Tradeoffs

- Parent-chain reads may trigger extra provider work if not cached well.
- Very long labels can overflow quickly; truncation and tooltips will be necessary.

#### Recommended Slice

1. Defer for now.
2. Revisit after user feedback on navigation and orientation issues.

### 5.4 Tree Refresh and Loading Feedback (Not Planned for Now)

**Status:** Not Planned for Now (still valid)

This proposal mainly serves to communicate background activity to the user. That is not the current product goal for the Inspector, so this item is not planned for now.

If this ever becomes necessary, prefer a minimal, action-local signal near the triggered interaction instead of introducing a general background-activity layer across the tree.

### 5.5 Diagnostic Status Bar (Revisited)

**Status:** Partially Implemented

#### Problem

The previous binary busy/idle-only bar was too limited once async activity and search feedback became core to the workflow.

#### Goals

- Keep the status bar lightweight.
- Surface high-value operational state (running/error/result status) without turning it into a debug console.

#### Proposed UX

- Keep busy/idle + spinner behavior for async work.
- Show concise status text in the status bar (search progress, result counts, cancellation, errors).
- Do not add low-value diagnostics (provider internals, exhaustive counters) unless requested.

#### Technical Design

- Keep the implementation minimal in `apps/inspector/src/view/status_bar.rs`.
- Feed status text from `InspectorViewModel` state instead of duplicating logic in the view.
- Continue to keep detailed diagnostics out of the default UI.

#### Risks and Tradeoffs

- Extra status details risk adding noise without helping the primary inspection workflows.

#### Recommended Slice

1. Keep current focused status payload (busy state + concise text).
2. Revisit only after user feedback indicates missing status information.

### 5.6 Command Palette and Shortcut Help

**Status:** Not Started (still valid)

#### Problem

The Inspector now has multiple commands, menus, toolbar actions, context menus, and keyboard shortcuts, but there is no unified discoverability surface.

#### Goals

- Make commands searchable.
- Eliminate label drift between menus, toolbars, and shortcuts.
- Reuse the same command metadata everywhere.

#### Proposed UX

- Add `Ctrl/Cmd+Shift+P` command palette.
- Add `F1` or `?` shortcut overlay/help dialog.
- Palette entries should show:
  - command label
  - shortcut
  - disabled reason when unavailable
- Initial palette scope:
  - search actions
  - reveal/highlight/refresh actions
  - expand/collapse actions
  - focus search
  - show about/help

#### Technical Design

- Replace ad hoc command duplication with a central command registry, e.g. a static slice of `CommandSpec`.
- `CommandSpec` should describe label, shortcut, category, enable predicate, and execution mapping.
- Generate menu items, toolbar labels, and palette entries from the same command definitions where practical.

#### Risks and Tradeoffs

- A full command registry is a moderate refactor, not just a UI addition.
- Over-generalizing too early may make simple commands harder to follow.

#### Recommended Slice

1. Introduce `CommandSpec` for existing AppCommand items.
2. Build shortcut help dialog from those specs.
3. Add the command palette on top.

### 5.7 Freeze and Snapshot Mode

**Status:** Not Started (still valid)

#### Problem

For fast-changing applications, live inspection can become unstable: selection jumps, attributes change while being read, and the tree no longer reflects what the user was trying to inspect.

#### Goals

- Allow stable inspection of transient UI states.
- Preserve the current live Inspector workflow for normal use.
- Make the difference between live and frozen inspection explicit.

#### Proposed UX

- Add a `Freeze` toggle in the toolbar or status area.
- When frozen:
  - show a clear banner or badge
  - disable automatic live refresh behavior
  - keep tree selection and attributes stable
- Later, support creating a named snapshot and comparing it to the live tree.

#### Technical Design

- Prefer a phased design:

Phase A: UI freeze
- Freeze UI-driven live updates and avoid background refresh/reveal side effects.
- Lowest implementation cost.

Phase B: immutable snapshot tree
- Capture a snapshot-specific data model detached from live provider objects.
- A snapshot should contain stable labels, ids, attributes, and child structure.
- Introduce a small backend abstraction only if necessary, e.g. `InspectorTreeSource` or a shared row data adapter.

Phase C: live-vs-snapshot diff
- Compare structure and attribute changes between two states.

#### Risks and Tradeoffs

- A real snapshot requires copying enough data to be useful without keeping live provider handles.
- Snapshot search adds scope quickly if XPath should run against frozen data.

#### Recommended Slice

1. Implement Phase A only.
2. Validate whether that solves enough real debugging problems.
3. Design immutable snapshot data only if Phase A is insufficient.

### 5.8 Documentation and Regression Coverage

**Status:** Partially Implemented

#### Problem

The Inspector has changed significantly, but `docs/inspector.md` still describes an older command and interaction set. Recent regressions around scroll-to-focus and text-selection context menus also show that interaction behavior is easy to break.

#### Goals

- Keep Inspector documentation aligned with implementation.
- Prevent regressions in high-value interaction paths.

#### Proposed Work

- Update `docs/inspector.md` to reflect:
  - menu bar
  - toolbar actions
  - shortcuts
  - attributes text actions
  - result reveal behavior
- Add a small manual regression checklist covering:
  - reveal from results scrolls tree to focused row
  - Escape in search keeps focus when expected
  - right-click attribute copy uses selection when present
  - shortcut labels in menus remain correct
- Add unit tests or focused integration tests where practical for:
  - command dispatch
  - ViewModel selection/reveal behavior
  - tree scroll trigger behavior

#### Risks and Tradeoffs

- GUI interaction tests are harder than ViewModel tests in egui.
- A manual checklist is lower confidence than automated tests, but still valuable.

#### Recommended Slice

1. Update docs immediately when the next Inspector work package lands.
2. Add a manual regression checklist.
3. Add automated tests for non-visual behavior first.

### 5.9 Async Execution Hardening and Performance

**Status:** Partially Implemented / Next

#### Problem

After migration to `egui-async`, most workflows are non-blocking, and search progress already appends only newly observed results on the UI side. Several async paths still need hardening for memory use, burst handling, stale side effects, startup latency, and cancellation behavior.

#### Goals

- Reduce per-frame overhead during long-running searches.
- Avoid duplicate ownership of large result sets.
- Make task lifecycle behavior consistent across all async task types.
- Improve cancellation and stale-result safety.
- Keep UI responsiveness predictable under heavy result volumes.
- Keep externally visible side effects (highlight/show/clear) ordered with the latest user intent.

#### Proposed Work

- Search progress streaming:
  - keep the current delta append behavior (`search_progress_seen`) as the baseline;
  - avoid keeping the full result list twice (`SearchResult.results` plus shared progress state);
  - transfer result ownership to the UI incrementally, or return only final metadata from the task.
- Bounded result ingestion:
  - cap how many newly available results are appended per frame;
  - keep remaining progress in a backlog so very large result bursts cannot monopolize one frame;
  - consider a user-visible result limit or warning for extremely broad XPath queries.
- Progress locking model:
  - avoid holding the progress mutex while cloning or appending large slices;
  - prefer drain/swap semantics or a channel-like queue to minimize contention between worker and UI frames.
- Task lifecycle unification:
  - standardize task start/poll/take/error/cancel handling patterns.
- Cancellation parity:
  - apply cancel-cooperative behavior consistently to reveal/highlight/selection where practical.
- Stale result protection:
  - extend request-id/epoch guards beyond selection where ordering matters.
- Side-effect ordering:
  - guard highlight and clear-highlight tasks with an epoch so an older task cannot clear or replace a newer highlight;
  - distinguish cancellable computation from already-started provider side effects, because `Bind::abort()` cannot reliably interrupt blocking provider calls.
- Repaint discipline:
  - centralize repaint decisions to avoid redundant repaint requests.
- Status modeling:
  - consider an explicit status enum instead of overloading free-form strings.
- Startup latency:
  - keep provider traversal behind the first rendered frames so startup cannot block on platform accessibility calls;
  - prefer renderer/backend configuration over pre-window provider traversal workarounds;
  - on Windows, keep GL out of the default `wgpu` backend mask because backend enumeration itself can trigger the slow UIA root traversal; leave GL available only through an explicit `WGPU_BACKEND=gl` override.
- Shutdown behavior:
  - cancel or ignore outstanding tasks when the app is closing;
  - ensure worker tasks do not publish stale UI state or provider side effects after the window is gone.
- Live node validity:
  - validate node-backed search results before delayed reveal/highlight actions;
  - handle invalidated provider nodes as normal stale data, not as task failures.
- Task instrumentation:
  - include task kind, request/epoch id, elapsed time, result count, and cancellation reason in debug logs.
- Async regression tests:
  - cover cancellation races and stale-result suppression in ViewModel/task tests.
- Windows COM safety notes:
  - keep explicit guardrails around UI-thread vs worker-thread runtime access in startup paths.

#### Current Implementation Status

This audit reflects the current Inspector code state after the `egui-async` migration.

| Area | Status | Notes |
| --- | --- | --- |
| Core `egui-async` migration | Done | Initial load, search, reveal, selection, and highlight use `Bind` tasks. |
| Search restart/cancel | Done | Starting a new search cancels the previous search; explicit cancel uses a shared flag and `Bind::abort()`. |
| UI-side search progress | Done | Search progress is drained incrementally from shared progress into visible results. |
| Search result ownership | Done | `SearchResult` now carries final metadata only; pending result items live in shared progress until drained by the UI. |
| Bounded result ingestion | Done | The UI drains at most a fixed batch of newly available results per frame. |
| Broad search result limit | Done | Inspector XPath search stops at 5000 results by default, with `--search-result-limit` and `PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT` allowing a custom count or `unlimited`. |
| Progress locking | Partial | Progress now uses `Mutex<VecDeque<_>>` with bounded drain; a fully lock-minimized queue or swap model is still optional. |
| Selection stale guard | Done | Selection details use `request_id` and ignore stale results. |
| Reveal stale guard | Done | Reveal tasks carry an epoch, check staleness during preload, and completed results are epoch-checked before applying UI state. |
| Highlight side-effect ordering | Partial | Highlight and clear-highlight tasks carry epochs and skip stale work before provider calls; already-started provider calls still cannot be forcibly interrupted. |
| Status feedback | Done | Search/result status is modeled with a small enum instead of free-form strings. |
| Startup provider guardrails | Partial | Initial root traversal is deferred until after the first rendered frames; Windows pre-window root preload was removed in favor of renderer/backend selection. The default Windows `wgpu` mask is Vulkan/DX12 only; GL remains opt-in through `WGPU_BACKEND=gl`. |
| Shutdown behavior | Open | No explicit close-time task cancellation/ignore policy is documented or implemented. |
| Live node validity | Done | Delayed reveal/highlight actions validate node-backed results before applying work. |
| Task instrumentation | Partial | Task logs now include more epochs and task context, but elapsed time, counts, and cancellation reasons are not yet consistent everywhere. |
| Async regression tests | Open | No Inspector async regression tests exist yet. |

#### Risks and Tradeoffs

- Over-abstracting task plumbing too early can reduce readability.
- Bounded per-frame ingestion can delay visibility of later results, but protects interaction latency.
- Cooperative cancellation depends on provider behavior; some blocking platform calls may only become ignorable, not interruptible.
- Additional async tests can be more involved than pure synchronous ViewModel tests.

#### Recommended Slice

1. Add explicit close/shutdown behavior for outstanding tasks.
2. Finish task instrumentation with elapsed time, result counts, limits, and cancellation reasons.
3. Add focused regression tests for high-volume search, cancellation races, stale side-effect suppression, and invalidated node-backed results.
4. Revisit shared task wrapper abstraction after those wins land.

## 6. Recommended Implementation Order

### Phase 1: Highest Value, Lowest Risk

- Results panel workflow and result context menu
- Attribute filter and pinned attributes
- Documentation refresh and regression checklist

**Status:** Mostly completed, with documentation/tests still open.

### Phase 2: Async Hardening and Responsiveness

- Result limit tuning for broad XPath queries
- Startup/shutdown task policy
- Remaining stale-result/cancellation consistency across async task types
- Async-focused regression coverage for ViewModel task orchestration

**Status:** Next recommended implementation phase.

### Phase 3: Strong UX Improvements with Moderate Refactor Cost

- Command metadata unification and shortcut help

### Deferred / Revisit After User Feedback

- Breadcrumb/path bar
- Diagnostic status bar

### Not Planned for Now

- Tree refresh/loading feedback

### Phase 4: Larger Architectural Additions

- Command palette
- Freeze mode Phase A
- Snapshot mode beyond Phase A

**Status:** Still valid, but follows async hardening work.

## 7. Suggested Decision Framework

When choosing the next item to implement, use these criteria:

- **Frequency**: does the user hit this workflow on every inspection session?
- **Disruption**: does the current behavior actively interrupt inspection?
- **Refactor cost**: how much shared UI architecture needs to move first?
- **Regression risk**: can we validate the behavior cheaply?

Based on the current post-refactor state, the strongest immediate candidate is **async hardening and performance** (Section 5.9), followed by command metadata unification and shortcut help.

## 8. Open Questions

- Should result browsing reveal on `Enter` only, or remain configurable?
- Do we want pinned attributes to persist across sessions or only during one run?
- Should the breadcrumb live above the tree, above attributes, or in the status/header area?
- Is Phase A freeze mode already sufficient for most debugging, or do we expect real snapshots quickly?
- Do we want a single command registry before adding more UI surfaces, or after one more feature pass?
- Should startup traversal stay purely frame-deferred, or should it gain a user-visible loading policy for very slow providers?
- Should async task status become a small enum before adding more task guards, or only after the next hardening slice proves the final states?

## 9. Next Step Recommendation

If only one further improvement is implemented next, continue **Async Execution Hardening and Performance** with startup/shutdown task policy. After that, finish task instrumentation and async regression coverage before taking the larger command-palette refactor.
