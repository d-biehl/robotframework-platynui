# Design

## Context

See proposal.md for why the demo exists and specs/demo-app/spec.md for what it must do. This section only records the facts from the existing code that shape the approach.

- **The repo already has a Qt Quick app:** the QML fixture `apps/test-app-qml`, with a thin PySide6 launcher and one QML file. Its README records verified bridge facts for PySide6 6.11 (`apps/test-app-qml/README.md`, sections "Verified platform facts"). These facts decide most of the demo's structure:
  - Every `Accessible.name` surfaces as `@Name` on UIA and AT-SPI.
  - `Accessible.id` (Qt ≥ 6.9) surfaces as the common `@Id` on AT-SPI. This is verified on Linux only. On Windows the fixture never wired it, and its UIA `AutomationId` stays empty.
  - A window's `@Name` is its title, because `Accessible` cannot attach to a `Window`. A window therefore has no `@Id`.
  - Native popup windows (`Popup.Window`) render, but never reach the AT-SPI tree. In-scene popups (`Popup.Item`) are visible on both platforms while they are open.
  - An in-scene `Dialog` takes its `@Name` from its title. It exposes modal state on AT-SPI only.
  - A `Rectangle` without `Accessible` wiring is absent from the tree, verified by the fixture's custom-controls chapter.
  - Lists and trees surface as `Group` (UIA) or `Filler` (AT-SPI), and rows as `ListItem`.
- **Table roles in the providers:**
  - AT-SPI maps `TableRow` and `TableCell` to `item:TableRow` and `item:TableCell` (`crates/provider-atspi/src/node.rs:973`, `:1007`).
  - UIA has no row or cell control types. Its mapping knows `DataItem` and `DataGrid` (`crates/provider-windows-uia/src/map.rs:117`).
  - How Qt Quick's bridge reports an item with `Accessible.role: Accessible.Row` / `Accessible.Cell` on UIA is **not verified**.
- **Toggle and selection state:**
  - `@ToggleState` and `@IsSelected` are defined in `crates/core/src/ui/attributes.rs:72-90`.
  - Only the Java providers set them (`crates/provider-java-jab/src/node.rs`, `crates/provider-java/src/agent/node.rs`).
  - UIA and AT-SPI expose those states only under `native:` (`crates/provider-windows-uia/src/map.rs:487-507`; AT-SPI `State`, `crates/provider-atspi/src/node.rs:2194`).
- **Cell text:** `control:Text` is sourced only from a real text interface and never from the name (spec `textcontent-pattern`). A QML cell therefore has an `@Text` only if the bridge gives that cell a text interface. That is also unverified.
- **Pure-Python packages:**
  - `packages/provider-java` is the model: `uv_build` backend, a `src/` layout, a console script (`packages/provider-java/pyproject.toml`).
  - `packages/*` are uv workspace members (`pyproject.toml:72-73`) and Cargo workspace members unless excluded (`Cargo.toml:2-13`).
  - Version sync picks up every `packages/**/pyproject.toml` automatically (`scripts/update-git-versions.py:59-70`).
  - The pure wheel is built in the CI job `build-wheels` (`.github/workflows/ci.yml:356-395`).
- **Launch processes in the acceptance lanes:**
  - The Qt fixtures sidestep the Windows launcher chain by starting the base interpreter with `__PYVENV_LAUNCHER__` (`tests/acceptance/qt/resources/testapp.resource:25-60`, `justfile:392`).
  - The demo must instead prove the documented recipe, which works *through* the launcher chain.

## Goals / Non-Goals

**Goals:**
- One Python package whose source reads like documentation for app developers. Python holds state and timing, QML holds the views, and each window lives in its own file with no abstraction layers.
- A tree shape that is chosen on purpose and verified on both platforms before any documentation depends on it.
- Acceptance suites that pin every spec scenario on the Windows and Linux lanes, and that start the demo exactly the way the documentation will.

**Non-Goals:**
- Blueprint conformance, canonical fixture names, and the catalog suite. The demo is a product, not a fixture.
- macOS, and generic Wayland compositors beyond what the platform support matrix already allows.
- Adding `@ToggleState` / `@IsSelected` to UIA and AT-SPI. That is a provider change of its own (see Risks).
- The documentation site (Astro/Starlight) and the tested documentation examples. That is a later change.
- Live language switching, persistence, native file dialogs, drag and drop.

## Decisions

### 1. Qt Quick on PySide6 ≥ 6.9, dependency `PySide6-Essentials`

The UI is QML with Qt Quick Controls. It is realistic for the point-of-sale and kiosk UIs Qt Quick is used for, and `Accessible.id` gives a clean `@Id`.

The alternatives:
- **Qt Widgets** reports a compound object path as `@Id` (`apps/test-app-qt/README.md`), which reads badly in tutorials.
- **egui** has no real dialog windows and no table semantics.

The package depends on `PySide6-Essentials` rather than the `PySide6` meta package, because the Addons wheel is not needed. That Essentials contains QtQml, QtQuick and QtQuickControls2 is an assumption. The first task verifies it, and falls back to `PySide6` if it does not hold. The floor is 6.9 because of `Accessible.id`.

### 2. Location `packages/demo`, import package `platynui_demo`

- **Why `packages/`:** the demo is a delivered wheel like `platynui-cli`, and `packages/` is where delivered Python packages live. `apps/` holds fixtures and binaries. A PEP 723 script like the fixtures cannot be installed with `uv tool install`.
- **Layout:**
  - `uv_build` backend with the `src/` layout.
  - QML files and images as package data under `platynui_demo/qml/`.
  - Console script `platynui-demo = "platynui_demo.__main__:main"`.
- **Workspace wiring:**
  - `Cargo.toml` `exclude` gains `packages/demo`.
  - `[tool.mypy] files` gains `packages/demo/src`.
  - The root project adds `platynui-demo` to its `dev` group as a workspace source, so `uv sync` installs it and the lanes find the command in `.venv`.

### 3. Python owns state and timing, QML owns views

- **Python:**
  - Plain Python modules hold everything that has no Qt in it: the product catalog, the seed history and stock, the price calculation, number formatting and input validation. Pytest can check these cheaply (the lowest test layer in `dev-docs/testing-strategy.md` §7).
  - Thin `QAbstractListModel` / `QObject` classes adapt that logic for QML: current order, orders, stock, kitchen queue, report.
  - A single delay helper multiplies the fixed delays by `--delay-factor` and drives `QTimer`s.
- **QML:**
  - One file per window or dialog: `Register.qml`, `CustomizeDialog.qml`, `PaymentDialog.qml`, `KitchenDisplay.qml`, `DailyReport.qml`, `Settings.qml`, `SignIn.qml`, `Splash.qml`.
  - A shared `DataTable.qml` for the three tables.
- **Translations:** a Python dictionary for `en` and `de`, exposed to QML as one context object. Qt's `qsTr` with `.ts`/`.qm` files was the alternative. It would add a `lrelease` build step and tooling for a two-language demo whose language is fixed at start.

### 4. Accessibility wiring conventions

- **Every interactive or documented element sets two things:**
  - `Accessible.name` to its visible text.
  - `Accessible.id` to an English kebab-case id such as `pay-button`, `order-total` or `orders-table`. Rows get `order-<n>` and `stock-<item>`.
  - Kebab-case matches the repo's other names and reads well in XPath. Real applications use every style, and the docs say so.
- **Menus and popups use `Popup.Item`,** because native popups are invisible on AT-SPI.
- **Two kinds of dialog, one per purpose:**
  - Customize, Payment and the confirmations are in-scene `Dialog`s, titled so that the title becomes the `@Name`.
  - Kitchen Display, Daily Report, Settings, Sign in and Please wait are separate `Window`s, because the documentation needs both kinds of dialog.
- **Window titles are ASCII (`PlatynUI Cafe - Register 1`).** In-app headings may keep the accent ("PlatynUI Café"). Unicode appears in data such as names on cups and German product names, where it is the lesson.
- **Product tiles** are custom-drawn items with `Accessible.role: Accessible.Button`, name and id. "Seasonal Special" has no `Accessible` attachment at all, which is the fixture-proven way to be absent.
- **Keypad and PIN:**
  - The keypad keys are custom buttons named by their label.
  - The PIN display is a masked field whose accessible name is its caption ("PIN"), never the digits.
- **Selection and checkboxes:**
  - Rows set `Accessible.selectable` / `Accessible.selected`, and Reorder checkboxes are real `CheckBox` controls. Their states are therefore reported natively on both platforms and become visible under the common attributes once a provider change maps them.
  - Until then, the observables the spec defines (Export caption, last-action text) are what the suites assert.

### 5. Tables are row delegates, not `TableView`

- **Structure:**
  - The three tables share one component: a `ListView` whose delegate is a row item (`Accessible.role: Accessible.Row`, `Accessible.id: order-<n>`), and whose children are cell items (`Accessible.role: Accessible.Cell`, `Accessible.name` = displayed text), in column order.
  - The column headers are a separate row of `Accessible.ColumnHeader` items above the list. They are not a child of the rows container, so `//*[@Id="orders-table"]/*` counts only order rows.
- **Why not `TableView`:** Qt Quick's `TableView` creates cells but no row items (assumption, checked in the spike). "Find the row by its cells" would then need row and column index attributes that PlatynUI does not have in common form.
- **Behaviour lives in Python:**
  - Selection (with Ctrl and Shift from the `TapHandler` modifiers), editing (a cell swaps its text for a `TextField` on double-click, and the Python model validates on Enter) and sorting of the report are Python-model operations.
  - QML only renders the state.

### 6. Startup and window lifecycle

- **`main()` runs these steps in order:**
  1. Parse the arguments. Invalid ones exit with argparse's usage error.
  2. On Linux, `os.environ.setdefault('QT_LINUX_ACCESSIBILITY_ALWAYS_ON', '1')`, before `QGuiApplication` exists.
  3. Pin one style (`Fusion`) with a fixed light palette on every platform. Dark mode is only a Setting.
  4. Create the application, load splash → sign-in → register as the options require, and enter the event loop.

  The fixed style differs deliberately from the fixture, which follows the system theme: the screenshots in the documentation must match what the reader sees.
- **Closing:** the register window's `onClosing` rejects the close while the order has lines, and opens the discard confirmation. Closing the register quits the application explicitly, so an open Kitchen Display cannot keep the process alive.

### 7. Acceptance suites start the demo through the documented recipe

- **Where the suites live:** `tests/acceptance/demo/`, one suite per spec area: launch, identity and language, register, payment, kitchen, orders, stock, secondary windows.
- **What the resource holds:** `demo.resource` holds only launch and teardown, like the fixture resources:
  1. `Start Process` on the demo command.
  2. `Wait Until Query` on the recipe expression.
  3. `Set Root` to `/app:Application[@ProcessId=${pid}]`.

  Teardown closes the main window through the UI, answers a discard confirmation if one appears, and waits until the application node is gone. `Terminate Process` on the handle is only the fallback.
- **Which command is started:**
  - It comes from `PLATYNUI_DEMO_COMMAND`, set by `scripts/platynui-robot-session.sh` and the `test-acceptance-windows` recipe. The default is the console script in the project `.venv`.
  - On Windows that script is a launcher, the same situation as `uv tool install`, so every lane run exercises the recipe behind a launcher.
- **Delay factor:** suites run with `--delay-factor 0.3`, which keeps the waits real but short. Only the scaling scenario uses 0 and 1.
- **Tags:** the directory is tagged `acceptance` and `real`, with no platform tag. Scenarios that are AT-SPI-specific (`item:TableRow`) carry `platform:x11` / `platform:wayland`, following the tag rules in `dev-docs/testing-strategy.md` §2.6.

### 8. The spike comes first and can change the spec

Before any production QML is written, a throwaway QML page is run against both providers. Its findings are recorded in this design, and the spec is adjusted before implementation starts. It checks:

- `Accessible.id` on UIA;
- how `Accessible.Row`, `Accessible.Cell` and `Accessible.ColumnHeader` surface on UIA and AT-SPI;
- whether a cell has `@Text` or only `@Name`;
- that `Accessible.selected` and `CheckBox` state appear natively;
- that a custom tile surfaces as a `Button` with name and id;
- that the unwired tile is absent;
- the `TableView` shape, for comparison.

Windows findings come from a real Windows desktop session, not Wine.

## Risks / Trade-offs

- **[Qt Quick's UIA bridge drops row and cell roles, or `Accessible.id`]** → The spike runs first. The role-independent scenarios (`*[3][@Name=…]`) still hold. Role-specific scenarios then stay AT-SPI-only (already tagged so), and a missing `@Id` on UIA would move id scenarios to documented platform deviations before implementation.
- **[Selection and toggle state not assertable through common attributes on UIA and AT-SPI]** → The suites assert the spec's observables. The provider gap is a separate change that also benefits every other Qt, GTK or UIA target, and the demo's `Accessible.selected` and `CheckBox` wiring is already in place for it.
- **[The launcher chain on Windows: `Terminate Process` may leave the demo running]** → Teardown ends the demo through its own UI and waits for the application node to disappear. `Terminate Process` is only a fallback. The Windows lane verifies this on a real desktop.
- **[Download size of PySide6]** → Acceptable for a demo. The documentation states it next to the install command.
- **[Qt's X11 plugin needs `libxcb-cursor0`]** → The Linux lanes already run Qt fixtures. The installation page of the documentation lists the system package.
- **[Documentation depends on names, ids and data]** → The spec is the contract. Changing a documented name or seed value needs a spec change and a documentation update in the same change.
- **[Timing flakiness]** → Every wait goes through `Wait Until …` keywords, never `Sleep`, and delays scale with `--delay-factor`.
- **[In-scene popup quirks (Escape closes one level)]** → Flows close their menus by activating an item, as the fixture's flows do.

## Migration Plan

- **Additive.** A new package, acceptance suites, recipes and a CI build step. No existing behaviour changes.
- **No native rebuild.** The package contains no Rust. The acceptance lanes need the usual non-mock build, which they already use.
- **Rollout:**
  1. Spike.
  2. Package skeleton and CI wheel.
  3. Screens.
  4. Suites on both lanes.
- **Rollback:** remove `packages/demo`, its `dev` dependency, the Cargo `exclude` entry, the mypy path, the recipes, the CI step and `tests/acceptance/demo`.

## Open Questions

- Whether cells also carry `@Text`. This is answered by the spike. The spec relies only on `@Name`, so the answer only decides which attribute the documentation shows.
- The exact dark-mode palette and the app icon. Cosmetic, and deferrable.
