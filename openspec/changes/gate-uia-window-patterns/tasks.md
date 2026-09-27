# Tasks

No test uses the taskbar, the shell or applications that Windows ships; UI Automation tests use the test window of `crates/provider-windows-uia/src/node.rs` (a child process shows it off screen), and acceptance tests use the repository's test apps.

## 1. Before the change

- [ ] 1.1 On the Windows lane with a real build (`just build-native`), read `@SupportedPatterns` of an open context-menu popup of the Qt test app and of an open popup of the QML test app, through a short BareMetal suite or `platynui-cli query` scoped to the app (design decision 9). Record here whether a popup's window advertises Activatable. If it does, verify by hand that activating its root with the Win32 window manager does not close it; if it does close, extend the root determination in 5.3 with a popup check and update design and spec before continuing.
- [ ] 1.2 Record the Windows lane's state before the change: run `just test-acceptance-windows` and `uv run --no-sync robotcode results log --level WARN --execution-messages`, and note the result and the duration of the egui, Qt and QML suites here. If `window-activation-state` still has its Windows verification open, run it in the same lane first, so that its scenarios are recorded on the old activation path.

## 2. Tests first — Robot Framework acceptance

Follow the `robot-test-style` skill.

- [ ] 2.1 In `tests/acceptance/egui/auto_activate.robot`, add "Highlight Of Another Element Leaves The Text Field Focused", "Take Screenshot Of Another Element Leaves The Text Field Focused" and "Raising A Background Window Keeps The Application's Own Focus" (spec `window-activation`). Replace the comment at `auto_activate.robot:107-108`, which describes the ungated behavior. Verify on the Windows lane that the three fail before the change (`just test-acceptance-windows --profile real-windows run --suite "Tests.Acceptance.Egui.Auto Activate"`).
- [ ] 2.2 In `tests/acceptance/egui/window_activation.robot`, add "Bring To Front On A Disabled Button Raises Its Window" and "A Pointer Action On A Disabled Element Still Raises Its Window", with a teardown that checks `chk-enable-conditional` again. Update the suite documentation, which says it runs only on X11 and the compositor although the Windows lane runs it too. Verify on the Windows lane that both fail before the change.

## 3. Tests first — Rust

- [ ] 3.1 Core: unit tests for `verify_pattern_instances` in `crates/core/src/ui/contract/testkit.rs` for the scenarios of `pattern-advertisement` (a conforming node, an unadvertised instance, an advertisement without instance, a marker with an instance, attribute-only patterns exempt, no action run). Verify with `just test-crate platynui-core` that they fail to compile.
- [ ] 3.2 Mock: a test in `crates/provider-mock/src/tests.rs` that runs the check on every node of the mock tree. Verify that it compiles once 4.1 exists and passes.
- [ ] 3.3 UI Automation, read-only, next to `listed_nodes_keep_their_parent_and_nothing_holds_its_children`:
  - the test window advertises and serves all eight window patterns;
  - every node listed below it serves and advertises none;
  - the pattern-instance check passes on the window and its subtree;
  - a listed button's nearest ancestor-or-self with an Activatable instance, and its `top_level_or_self()`, is the window, and the button has no Activatable instance;
  - the handle whose root is activated for a button is the window's (spec `uia-window-activation`);
  - the mirror check of `supported_patterns_attribute_mirrors_the_pattern_list` also on the window and a button.

  Verify on Windows with `just test-crate platynui-provider-windows-uia`: the gate, subtree, contract and ancestor tests fail before the change.
- [ ] 3.4 UI Automation: a unit test of the route decision (handle and manager → window manager; handle without manager → the missing-manager error; no handle → UIA focus), and a test that activating the test window without an injected window manager fails with that error while the foreground window stays the same. Verify that they fail to compile before the change. Do not run the old activation against the test window: it would change the foreground.
- [ ] 3.5 UI Automation, ignored: activating the test window through its Activatable instance with the real Win32 window manager (from `platform_factories()`, `platynui-platform-windows` as a dev-dependency) leaves the window manager reporting it active. Add `-p platynui-provider-windows-uia` to the ignored-only run of `test-acceptance-windows` (`justfile:385`). Verify that `cargo nextest run -p platynui-provider-windows-uia --run-ignored ignored-only` lists it.
- [ ] 3.6 Java: in `crates/provider-java/tests/live_fixture.rs`, add the pattern-instance check next to the inline loops at `:319-333` and `:1057-1068`, which stay. Verify with `cargo check -p platynui-provider-java --tests`.

## 4. Core and platform

- [ ] 4.1 Implement `verify_pattern_instances` and the `ContractIssue` variants `PatternWithoutInstance`, `InstanceWithoutAdvertisement` and `MarkerWithInstance`, with the list of action patterns next to `declare_action_pattern!` (`crates/core/src/ui/pattern.rs`). Correct the documentation of `validate_control_or_item` (`crates/core/src/ui/contract.rs:16-24`) to what it checks, pointing to the new check. Verify that 3.1 and 3.2 pass (`just test-crate platynui-core`, `just test-crate platynui-provider-mock`).
- [ ] 4.2 In `crates/platform-windows/src/window_manager.rs`, log a refused foreground change once per episode per window (design decision 6), and correct the comments that call `NativeWindowHandle` UIA property 30005 (it is 30020). Verify with `just test-crate platynui-platform-windows` and a unit test of the episode rule if the episode logic has its own function.

## 5. UI Automation

- [ ] 5.1 Override `set_window_manager` in `WindowsUiaProvider` (set-once, like `set_java_classifier`) and pass the manager to every node the provider creates: `ElementAndAppIter`, `ApplicationNode` and `AppWindowIter`, `element_at_point` with `attach_ancestor_chain`, and `ElementChildrenIter` from its parent. Give the tests a way to build the test window's node with and without a manager. Verify with `just clippy` and `just test-crate platynui-provider-windows-uia`.
- [ ] 5.2 Gate the window branch of `pattern_by_name` on `has_window_surface()`, with one helper for both methods; make `has_window_surface` short-circuit; add `IsWindowPatternAvailable` and `IsTransformPatternAvailable` to the traversal cache request and fill the value in `populate_cached_properties`. Verify that the gate, subtree, contract and ancestor tests of 3.3 pass.
- [ ] 5.3 Rewrite the Activatable action (design decisions 2 and 4): find the native handle of the element or its nearest ancestor with one (bounded raw-view walk), take its `GA_ROOT`, and activate it through the window manager, then wait until it reports the window active or for at most a second; fail with the missing-manager error without a manager; set UIA focus only when no handle exists. Keep the debug trace. Document at the code that a WindowId of the Win32 window manager is a window handle. Verify that 3.3 and 3.4 pass, and that 3.5 passes on a Windows host.

## 6. Documentation

- [ ] 6.1 Update:
  - `dev-docs/platform-windows.md` (UIA activation through the injected window manager, `GA_ROOT`, focus only without a handle, the nested-window limitation);
  - `dev-docs/architecture.md`: the Activatable row, whose UIA column at `:487` describes `SetFocus`; the promise at `:662` that activation gives the window the keyboard focus; the pattern-honesty rule at `:294`, which exempts markers and attribute-only patterns;
  - `dev-docs/testing-strategy.md` §2.2 (`verify_pattern_instances` next to the other testkit checks);
  - the BareMetal docs of Activate Window (`src/PlatynUI/BareMetal/__init__.py:2261`), Bring To Front (`:2365`), the window-control section (`:713`) and "Bringing windows to the front" (`:677-700`): the window becomes the active window, and PlatynUI moves no focus inside it.

  Flag the wording "WindowSurface pattern" in the documentation of `top_level_or_self` (`crates/core/src/ui/node.rs:139-141`). Verify by reading and with `just check`.

## 7. Verification

- [ ] 7.1 Run `just check`, `just test` on a Windows host (it runs the UI Automation tests), `just test-python` and `just test-baremetal`, then `just build-native`. Verify that everything is green.
- [ ] 7.2 On Windows, run `just install-provider-java`, `just test-acceptance-windows` and `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - everything is green, including 2.1, 2.2, 3.5 and the Java live tests with 3.6;
  - there is no WARN or ERROR from PlatynUI;
  - the suite durations compared with 1.2 show no stall.

  Record the outcome here.
- [ ] 7.3 CI runs the X11 and compositor lanes on push. Verify there that the scenarios of 2.1 and 2.2 are green and the logs have no WARN or ERROR from PlatynUI, and record the run here.

## 8. Commit (only when the user asks)

- [ ] 8.1 Commit in reviewable steps. Each step carries the tests it turns green, so each builds, passes lint and passes its tests on its own:
  - the testkit check, with 3.1, 3.2 and 3.6;
  - the window manager's warning;
  - the UIA injection, gate and route, with 3.3–3.5 and the acceptance tests of 2.1 and 2.2;
  - the docs.

  Subjects ≤ 72 characters, no `!`. The UIA commit lists the behavior changes of the proposal in its body.
