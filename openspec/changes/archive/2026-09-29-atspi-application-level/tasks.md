# Tasks

The behavior is Linux AT-SPI, so the Rust unit tests and both Linux acceptance lanes cover it. The Avalonia shape that triggered the change is checked by hand in 4.3, because Avalonia is no lane fixture (design D8).

## 1. Tests first

- [x] 1.1 In `crates/provider-atspi/src/node.rs` (tests module), replace the three `map_role_with_interfaces_*` tests (:2508-2531) with a table test of the classification function of level and role (design D2). The table has these rows:
  - application level, `Role::Application` → `app:Application` (spec: *A root with the application role is app:Application*);
  - application level, `Role::Frame` → `app:Frame` (*A root with another role keeps that role*);
  - application level, `Role::Invalid` → `app:Unknown` (*A root whose role cannot be read stays at the application level*);
  - below the application level, `Role::Frame` → `control:Frame`. This is Avalonia's window; the interface is no input (*An Avalonia window is a frame*);
  - below the application level, `Role::Application` → `control:Application` (*An application role inside a tree is a control*).

  Change `map_role_application` (:2436-2441) to expect `control:Application` (design D3). Add a row for the application level without any interface information, since the function takes none (*A registered application without the Application interface stays at the application level*).

  Verify: `just test-crate platynui-provider-atspi` fails to compile or fails these tests before 2.2.
- [x] 1.2 In the same module, next to the `application_id` tests (:2342-2363), test the level-based decision of design D4:
  - Below the application level, a peer with a known process ID yields no process attributes and no process-derived `Id`, and the accessible-id is used instead.
  - At the application level, the existing behavior holds.

  This covers *Process attributes and Id follow the level, not the interface*. Verify: the tests fail before 2.1.
- [x] 1.3 In `crates/runtime/src/runtime/window.rs` (tests module, :159 ff.), add two tests of `top_level_window_for`. Build the nodes from `test_fixtures.rs`; add a fake there that takes a namespace and a role if none fits.
  - An `app` node with the role `Frame` and a child that exposes `Activatable` returns that child (spec: *An application node with another role still leads to its window*).
  - A `control:Application` node without an `Activatable` ancestor and with an `Activatable` child returns `None` (*A control named Application does not lead to a window*).

  Verify: `just test-crate platynui-runtime` fails the first test before 2.3 and passes the second one already.
- [x] 1.4 Add a test case to `tests/acceptance/egui/hit_test.robot`, following the `robot-test-style` skill. It hit-tests the "Click Me" button like the first case, and asserts about the resolved element:
  - it has exactly one ancestor in the `app` namespace;
  - the suite's window is a direct child of that ancestor;
  - the window exposes `Activatable`, and the resolved button does not.

  This covers *The hit-test reaches the window through one application node* and the egui side of *Only the children of the application level are top-level windows*. The case stays provider-independent: it uses no `native:` attribute, and role wildcards where the role differs between providers. Verify: `just headless=true test-acceptance-x11 --suite '*.Egui.HitTest'` passes before the change, as a regression guard.

  Run of 2026-09-28: passes before the change (4/4). The case reads the chain with the node's `ancestors()` and not with `ancestor::app:*`: the hit-test builds the application node without a parent, and the runtime's XPath adapter makes a node without a parent a document node (`crates/runtime/src/xpath.rs:466-467`), so the axis step never matches it. This was already so before the change and is left alone here. The pattern names in `SupportedPatterns` are the full names (`org.platynui.patterns.Activatable`).

## 2. Implementation

- [x] 2.1 In `crates/provider-atspi/src/node.rs`, give `AtspiNode` the application-level marker with a dedicated constructor (design D1), and use it at the two sites that build registry applications: `get_nodes` (`crates/provider-atspi/src/lib.rs:324`) and the hit-test (`lib.rs:405`).
  - Replace `is_application()` in `process_attributes` (:253-256) and `id()` (:286-296) with the level (design D4), and remove `is_application()`.
  - Keep the `native:Application.*` properties tied to the interface (:1094-1102).

  Verify: 1.2 passes.
- [x] 2.2 Replace `map_role_with_interfaces` (:996-1001) with the classification function of design D2, and use it in `resolve_role` (:147-170) and in the seeding of `get_nodes` (`lib.rs:333-336`). Map `Role::Application` to `control` in `map_role` (:937, design D3).
  - Update the doc comments that name "the `Application` accessible" as the parent of a top-level window: `is_window_surface` (:198-225), `is_transient_popup` (:227-234) and `resolve_extents_via_parent_chain` (:1495-1497). They now name the application level.

  Verify: 1.1 passes, and `just test-crate platynui-provider-atspi` is green.
- [x] 2.3 In `crates/runtime/src/runtime/window.rs`, let `top_level_window_for` recognise the application node by the `app` namespace alone (:50), and update its doc comment (:41-43, design D6). Verify: 1.3 passes, and `just test-crate platynui-runtime` is green.

## 3. Documentation

- [x] 3.1 In `dev-docs/platform-linux.md` §2 (Node Model, :112-115), replace "`app:Application` nodes for processes with the Application interface" with the rule of this change:
  - the registry's applications are the application level in `app`, with the role their object reports;
  - below that level the Application interface only shows up as native attributes;
  - a node with the role `application` there is `control:Application`.

  Keep it as short as the surrounding bullets and point to the spec for the details. Verify by reading the section against `specs/atspi-application-level/spec.md`.

## 4. Verification

- [x] 4.1 Run `just check`, `just test-crate platynui-provider-atspi` and `just test-crate platynui-runtime`. All are green.

  Run of 2026-09-28: `just check` clean, provider 109/109, runtime 167/167. Since the role no longer depends on the interfaces, `resolve_role` no longer prefetches them; `ClearableCell::is_set`, used only there, is now test-only.
- [x] 4.2 Run the full Linux acceptance lanes, since every egui, Qt and QML suite selects its application under `app:`: `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor`. Both recipes build the native package first. Inspect the results with `robotcode results`; both lanes are green, including the new case from 1.4.

  Run of 2026-09-28: X11 lane 86/86, compositor lane 87/87, both headless; the new hit-test case passes on both.

  Run of 2026-09-29, after rebasing onto `application-process-attributes`. `6c85cb8` moved the seeding in `get_nodes` into `AtspiNode::seed_application`, which now takes the role through `set_role`. `just check` is clean, and `just test` passes 2460/2460. The X11 lane passes 94/94 and the compositor lane 95/95, both headless.
- [x] 4.3 Check an Avalonia 12 application by hand, inside `scripts/startxsession.sh --backend headless -- <script>`:
  - Create the app from the Avalonia MVVM template with Avalonia 12.1.2, and start it with `dotnet run`.
  - Read the tree with `target/debug/platynui-cli query` and the raw bus with `busctl --address="$AT_SPI_BUS_ADDRESS"`.
  - Expect:
    - `/*` is `app:Application "Avalonia Application"`;
    - `/*/*` is `control:Frame` named after the window, with the window patterns, without `@ProcessId` and `app:*`, and with `Application` in `native:Accessible.Interfaces`;
    - the frame's children expose no window pattern and no `@IsActive`;
    - `platynui-cli window --list '//control:Frame'` shows the frame with its window state;
    - `platynui-cli element-at-point` on a point inside the window returns a chain with exactly one `app` node.

  This covers *An Avalonia window is a frame* and *Only the children of the application level are top-level windows*. Record the output in the run notes.

  Run of 2026-09-28 with the MVVM template app (`GetStartedApp`, Avalonia 12.1.2, .NET 10), every expectation met. Note that `target/debug/platynui-cli` is built by the package `platynui-cli-bin`; `cargo build -p platynui-cli` builds `platynui-cli-rs` and leaves it stale.
  - Bus: the registry lists one application, `:1.8` at `/org/a11y/atspi/accessible/root`, role `application`, interfaces `Accessible`, `Application`. Its one child `/net/avaloniaui/a11y/1` has the role `frame` and the interfaces `Accessible`, `Application`, `Component`.
  - `/*` is `app:Application "Avalonia Application"` with `@ProcessId` and the `app:*` block; `count(//app:*)` is `1` (before the change: `2`).
  - `/app:*/*` is `Frame "GetStartedApp"` (`control`), with the eight window patterns and `@IsActive = true`, `count(@ProcessId)` and `count(@app:*)` both `0`, and `native:Accessible.Interfaces = ["Accessible","Application","Component"]`.
  - Its children `Panel "WindowChrome"` and `Panel "Panel"` have `@SupportedPatterns = []`, and `count(/app:*/control:Frame/*/@IsActive)` is `0`.
  - `window --list '//control:Frame'` lists `Frame "GetStartedApp"` with its state and bounds (`5,24 1440x737`); before the change it matched nothing.
  - `element-at-point 725 392` resolves `Panel 'TextPresenter'` inside a `TextBox`, whose chain ends in `Frame 'GetStartedApp'` and then `Application 'Avalonia Application'`, the only `app` node.
