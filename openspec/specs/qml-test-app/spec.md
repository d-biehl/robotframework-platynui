# qml-test-app Specification

## Purpose

The Qt Quick/QML row of the fixture technology matrix (`dev-docs/testing-strategy.md` §5) and the first full instance of the fixture blueprint (§5.1). Qt Quick renders its own scene graph: accessibility comes from QML `Accessible` attached properties routed through Qt's bridge (UIA on Windows, AT-SPI on Linux), menus/popups are in-scene items by default, and dialogs exist in two faces (native child window, in-scene overlay). The fixture (`apps/test-app-qml`, PySide6, thin PEP 723 `main.py` + `Main.qml`) implements the blueprint core tier under the canonical names and carries the canonical catalog test set's first onboarding (`tests/acceptance/qml`); verified platform deviations are documented in the app README and reflected in the blueprint spec.

## Requirements

### Requirement: Blueprint-conforming core catalog in QML
The fixture SHALL implement the `test-app-blueprint` core-tier catalog as a Qt Quick scene (QtQuick.Controls) launched by a thin PySide6 `main.py`: every core-tier control present under its canonical name, names wired through `Accessible` attached properties (`Accessible.name`, with explicit roles where Controls do not set them), and the blueprint's action observables (`clicks-<n>` counter on `status-label`, the `last-action-<ident>` report label for menu items and dialog buttons) functional. Surfaced names and roles SHALL be verified against the real accessibility tree (Inspector or `Get Attribute`) on both Windows/UIA and Linux/AT-SPI before the acceptance suites encode them. The extended tier is out of scope for this change and follows without renaming anything.

#### Scenario: Core tier enumerable on Windows and Linux
- **GIVEN** the fixture is running
- **WHEN** the platform provider (UIA on Windows, AT-SPI on Linux/X11) walks the tree under `main-window`
- **THEN** every core-tier control resolves under its canonical name, and no interactive control reports an empty or duplicate accessible name

#### Scenario: Click counter observable through the scene graph
- **WHEN** `button-basic` is activated twice via real pointer input
- **THEN** `status-label`'s text and accessible name end with `clicks-2` on the accessibility tree of both platforms

#### Scenario: Bridge gaps become documented deviations, not silent failures
- **WHEN** a required state (e.g. checkbox toggle state, modal state) does not surface through Qt Quick's accessibility bridge on a platform
- **THEN** the fixture README documents the deviation, the fixture adds the blueprint's name-based fallback observable where prescribed, and the corresponding catalog test is a documented skip on that platform

### Requirement: Fixture CLI per blueprint plus QML-specific flags
The fixture SHALL implement the blueprint CLI contract (`--title` with default `PlatynUI QML TestApp`, `--auto-close <seconds>`, `--open-modal`, usage + non-zero exit on unknown arguments) and additionally: `--app-id <id>` (X11 WM_CLASS / Wayland app_id, default `org.platynui.test.qml`), `--popup-mode {inscene,native}` (default `inscene`), and `--log-level {error,warn,info,debug}`. Defaults SHALL produce the conforming catalog with in-scene popups.

#### Scenario: Blueprint contract honored
- **WHEN** the fixture starts with `--title "QML Catalog" --auto-close 5`
- **THEN** `main-window`'s title is "QML Catalog" and the process exits with code 0 shortly after 5 seconds without interaction

#### Scenario: Unknown argument fails fast
- **WHEN** the fixture starts with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

### Requirement: Dual popup modes
In the default `inscene` mode, the menu bar's menus and the context menu SHALL be Qt Quick in-scene popups (items inside the window, Qt Quick's default); in `native` mode (Qt ≥ 6.8 `popupType`/native menus) the same menus SHALL be native popup windows — verified reality: they render as such on all platforms, but on Linux (X11 and the PlatynUI compositor) their contents never reach AT-SPI (no tree node, no hit-test; unlike Qt Widgets' native `QMenu`), so the tree-driven native-mode coverage is platform-scoped to Windows per the documented-deviation rule and the fixture README records the finding. In both modes the catalog menu items SHALL be reachable in the accessibility tree while their menu is open and SHALL report `last-action-<ident>` when activated through real input. The two modes SHALL NOT change any canonical name.

#### Scenario: In-scene context menu is drivable
- **GIVEN** the fixture runs in default `inscene` mode
- **WHEN** the context menu is opened via right-click and `ctx-copy` is activated via real pointer input
- **THEN** an element named `last-action-ctx-copy` is resolvable, and the popup was an in-scene item (no new native top-level window appeared for it)

#### Scenario: Native mode produces native popup windows
- **GIVEN** the fixture runs with `--popup-mode native` on a platform where native popups work (verified: Windows)
- **WHEN** the `menu-file` menu is opened
- **THEN** the open menu is exposed as (or inside) a native popup window and `menu-file-new` activates with the `last-action-menu-file-new` observable

#### Scenario: Submenu reachable in in-scene mode
- **WHEN** the in-scene context menu's submenu `ctx-more` is opened
- **THEN** `ctx-sub-alpha` and `ctx-sub-beta` are resolvable while open, and activating one fires its `last-action-<ident>` report

### Requirement: Both QML dialog faces
`dialog-modeless` SHALL be a real child `Window` (a native top-level window parented to the main window); `dialog-modal` SHALL be an in-scene modal `Dialog` (Qt Quick overlay) opened via `--open-modal`. Each SHALL contain its `<dialog-ident>-button` and `<dialog-ident>-label` with the blueprint's last-action report observable, so pointer correctness is provable for a native child window and an in-scene overlay alike. If the in-scene dialog's modal state does not surface on a platform's tree, the deviation rule of the core-catalog requirement applies.

#### Scenario: Click lands in the native child window
- **WHEN** `dialog-modeless-button` is clicked at its reported bounds
- **THEN** `last-action-dialog-modeless-button` is resolvable

#### Scenario: In-scene modal dialog present without interaction
- **WHEN** the fixture starts with `--open-modal`
- **THEN** `dialog-modal` and its children are resolvable in the tree without any prior input, and clicking `dialog-modal-button` at its reported bounds makes `last-action-dialog-modal-button` resolvable

### Requirement: Custom-controls chapter implemented
The fixture SHALL implement the blueprint's custom-controls chapter: `custom-button` as a self-drawn item (`Rectangle` + `MouseArea`) with manually wired `Accessible` properties (role Button, name, focusable) updating `custom-status-label` to `clicks-<n>`, and one drawn element without any `Accessible` wiring, documented in the README as the expected-absent negative case.

#### Scenario: Hand-built control drivable like a native one
- **WHEN** `custom-button` is activated twice via real pointer input
- **THEN** `custom-status-label`'s text and accessible name end with `clicks-2`

#### Scenario: Unwired element is absent
- **WHEN** the tree under `main-window` is searched for the documented unwired element
- **THEN** it is not resolvable by name, and the suite asserts that absence

### Requirement: Catalog suite onboarded on both lanes
This change SHALL deliver the blueprint's canonical catalog test set as the self-contained suite `tests/acceptance/qml/catalog.robot` (the blueprint's reference implementation): test bodies written directly against `PlatynUI.BareMetal`, locators relative to the fixture instance the launcher pins as the query root, launch configuration from `PLATYNUI_TEST_APP_QML_*` environment variables. The suite SHALL run in the Windows lane (`just test-acceptance-windows`) and the Linux lane (`scripts/platynui-robot-session.sh`), tagged per the existing `real`/platform conventions; QML-specific coverage (popup modes, custom controls, in-scene dialog) SHALL live in QML-suite files beside it. Known Qt Quick bridge limitations SHALL appear as documented skips or platform-scoped tags.

#### Scenario: Catalog suite green from launch configuration alone
- **WHEN** `catalog.robot` runs in either lane with the fixture launch env vars set
- **THEN** the canonical catalog tests execute against the QML fixture, and each non-passing catalog test is a documented skip or platform-scoped test naming its limitation

#### Scenario: Missing launch configuration skips actionably
- **WHEN** the suite runs without `PLATYNUI_TEST_APP_QML_*` set
- **THEN** the QML suites skip (or fail setup) with a message naming the missing variables and the recipe/script that provides them, instead of hanging or failing obscurely
