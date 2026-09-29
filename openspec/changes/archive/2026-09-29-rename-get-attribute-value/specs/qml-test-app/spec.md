# Spec Delta

## MODIFIED Requirements

### Requirement: Blueprint-conforming core catalog in QML
The fixture SHALL implement the `test-app-blueprint` core-tier catalog as a Qt Quick scene (QtQuick.Controls) launched by a thin PySide6 `main.py`: every core-tier control present under its canonical name, names wired through `Accessible` attached properties (`Accessible.name`, with explicit roles where Controls do not set them), and the blueprint's action observables (`clicks-<n>` counter on `status-label`, the `last-action-<ident>` report label for menu items and dialog buttons) functional. Surfaced names and roles SHALL be verified against the real accessibility tree (Inspector or `Get Attribute Value`) on both Windows/UIA and Linux/AT-SPI before the acceptance suites encode them. The extended tier is out of scope for this change and follows without renaming anything.

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
