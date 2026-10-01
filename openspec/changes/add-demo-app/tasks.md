# Tasks

## 1. QML accessibility spike (decides the spec before anything is built)

- [ ] 1.1 In a scratch venv, install `PySide6-Essentials>=6.9` alone and import `QtQml`, `QtQuick` and `QtQuickControls2`. Record the result in design.md Decision 1, and switch the dependency to `PySide6` if an import fails.
- [ ] 1.2 Write a throwaway spike page outside the repo containing:
  - `Accessible.id` on a button;
  - a `ListView` table whose row delegates use `Accessible.Row`, whose cells use `Accessible.Cell` (name = text) and whose header uses `Accessible.ColumnHeader`;
  - a `TableView` variant;
  - rows with `Accessible.selectable` / `Accessible.selected`;
  - a `CheckBox` inside a row;
  - a custom-drawn tile with `Accessible.role: Accessible.Button`;
  - one tile without any `Accessible`.

  Verify that the page starts on the project venv.
- [ ] 1.3 On Linux, run the spike under the X11 lane session and under the PlatynUI compositor. Record for each element: its role and namespace, `@Id`, `@Name`, whether `@Text` is present, and the native selection and check state. Read these with `Query` in `robotcode repl` or with the Inspector, and write them into a "Spike findings" section in design.md.
- [ ] 1.4 Run the same spike on a real Windows desktop session (the VM's desktop port, not Wine) and record the UIA facts in the same section.
- [ ] 1.5 Adjust specs/demo-app/spec.md and design.md Decisions 4–5 to every finding that differs from the assumptions. Examples: the UIA row and cell roles, `@Id` on UIA, `TableView` versus row delegates. Verify with `openspec validate add-demo-app --strict`.

## 2. Package skeleton and repository wiring

- [ ] 2.1 Create `packages/demo`:
  - `pyproject.toml` with the `uv_build` backend, `src/platynui_demo/`, `__version__.py` and `__main__.py`;
  - `qml/` as package data;
  - the console script `platynui-demo`.

  Verify that `uv build --wheel packages/demo -o dist` produces a wheel that contains the QML files.
- [ ] 2.2 Wire the workspace:
  - `packages/demo` into the `Cargo.toml` `exclude`;
  - `packages/demo/src` into `[tool.mypy] files`;
  - `platynui-demo` into the root `dev` group plus `[tool.uv.sources]`.

  Verify that `uv sync` puts the `platynui-demo` command into `.venv`, that `cargo metadata --no-deps` does not list the package, and that `scripts/update-git-versions.py` rewrites its version.
- [ ] 2.3 Add the `just` recipes `run-demo *ARGS` and `build-demo-wheel`, and add the latter to `build-all-wheels`. Add a step to the CI `build-wheels` job that builds the demo wheel into the uploaded artifact. Verify that both recipes work locally.

## 3. Tests first, derived from the spec

- [ ] 3.1 Add pytest unit tests under `packages/demo/tests` for the Qt-free core. They fail until 4.1 exists. They cover:
  - prices: Cappuccino L + oat + shot + 2× Croissant = 9.20; Latte M − 20 % = 0.72 / 2.88; change from 10.00 = 0.80;
  - en/de number formatting;
  - quantity validation (`ten`, `-1`, `12`);
  - every seed invariant from the spec's data requirement;
  - argument parsing: unknown option, `--lang fr` and `--delay-factor -1` give a non-zero exit, and `--register 2` is accepted.
- [ ] 3.2 Add `tests/acceptance/demo/`:
  - `__init__.robot`, tagged `acceptance` and `real`, launching nothing;
  - `resources/demo.resource`: start `${PLATYNUI_DEMO_COMMAND}`, `Wait Until Query` on the documented recipe expression, `Set Root` by `@ProcessId`, and a teardown that closes the main window, answers a discard confirmation and waits until the application node is gone.

  `PLATYNUI_DEMO_COMMAND` is exported by `scripts/platynui-robot-session.sh` and by the `test-acceptance-windows` recipe, and defaults to the `.venv` console script. Verify with `robotcode analyze` and a dry run that the resource resolves.
- [ ] 3.3 Write one Given/When/Then test per spec scenario:
  - `launch.robot`:
    - installed command, register number, unknown option, invalid value;
    - "visible to AT-SPI", started with an environment copy that lacks the Qt accessibility variables, tagged Linux only;
    - the recipe behind a launcher;
    - two registers.
  - `identity.robot`: id survives the language, the unexposed tile, German formatting, same data on every start, delay factor.
  - One suite each for register, payment, kitchen, orders (AT-SPI role scenario tagged Linux only), stock, and the secondary windows (report, settings, splash, sign in, closing).

  Verify with `robotcode analyze` and a dry run that every scenario of the spec has exactly one test.

## 4. Implementation

- [ ] 4.1 Implement the Qt-free core: catalog and seed data, pricing, formatting, the en/de string table, validation, and the delay factor. Verify that the 3.1 tests pass under `uv run pytest packages/demo/tests`.
- [ ] 4.2 Implement `main()`:
  - argument parsing;
  - `QT_LINUX_ACCESSIBILITY_ALWAYS_ON` via `setdefault` on Linux, before `QGuiApplication` exists;
  - the fixed `Fusion` style and light palette;
  - the window sequence splash → sign in → register;
  - the Qt models (current order, orders, stock, kitchen queue, report).

  Verify that `just run-demo` opens the titled main window and that the launch.robot argument tests pass on the X11 lane.
- [ ] 4.3 Implement `DataTable.qml` per design Decision 5 (row delegates, cells, a separate header row, selection and editing hooks). Verify that the orders-structure tests pass on the X11 lane.
- [ ] 4.4 Implement `Register.qml`:
  - menus as in-scene popups, the tab bar and the category tree;
  - tiles, including the unwired "Seasonal Special";
  - the order panel with the line context menu and the discount submenu;
  - totals, the status bar, Ctrl+N and Ctrl+K;
  - the close confirmation.

  Verify that register.robot and the closing test pass.
- [ ] 4.5 Implement `CustomizeDialog.qml` and `PaymentDialog.qml` (cash change, masked PIN, terminal wait, the declined PIN `0000`). Verify that payment.robot passes.
- [ ] 4.6 Implement `KitchenDisplay.qml` (tickets as named groups, sequential brewing, Serve). Verify that kitchen.robot passes.
- [ ] 4.7 Implement the Orders tab (selection with Ctrl and Shift, Export caption, context menu, refund with and without confirmation). Verify that orders.robot passes.
- [ ] 4.8 Implement the Stock tab (On hand editing with validation, Reorder checkboxes, Place reorder). Verify that stock.robot passes.
- [ ] 4.9 Implement `DailyReport.qml`, `Settings.qml`, `Splash.qml` and `SignIn.qml` (custom keypad, PIN never exposed). Verify that the secondary-windows suite passes.
- [ ] 4.10 Complete the German strings for every visible text. Verify that identity.robot passes with `--lang de`, and that a sweep in German finds no element whose `@Name` is still English (ids excluded).

## 5. Documentation of the package

- [ ] 5.1 Write `packages/demo/README.md`: install, run, the options, the deliberate teaching cases, and the rule that documented names, ids and data change only through the spec. Verify that it matches `platynui-demo --help`.
- [ ] 5.2 Update the orientation docs. Verify by reading each diff:
  - AGENTS.md: list `packages/demo` in Quick Orientation;
  - CONTRIBUTING.md: the new recipes;
  - root README.md: "Package docs";
  - `dev-docs/testing-strategy.md` §5: one sentence that the demo is a product with its own acceptance suites, not a fixture.

## 6. Verification

- [ ] 6.1 Run `just check` and `just test-python`. Both pass, the demo's unit tests included.
- [ ] 6.2 Run `just test-acceptance-x11` and `just test-acceptance-compositor`. All demo suites pass, and the existing suites are unaffected.
- [ ] 6.3 Run `just test-acceptance-windows` on the real Windows desktop. All demo suites pass, including the recipe behind the `.venv` launcher. This stays open until it has been run on real Windows.
- [ ] 6.4 Run `just build-demo-wheel`, then `uv tool install dist/platynui_demo-*.whl` in a clean environment. Start `platynui-demo`, and check the launch recipe once by hand against that tool install on Linux and on Windows.
- [ ] 6.5 Run `openspec validate add-demo-app --strict`, and tick every task above.
