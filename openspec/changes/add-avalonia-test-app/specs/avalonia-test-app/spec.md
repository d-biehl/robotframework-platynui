# Spec Delta

## Purpose

The Avalonia fixture application: the Avalonia row of the fixture technology matrix. It is a blueprint-conforming catalog built from Avalonia's standard controls. On Linux it is driven through AT-SPI on Avalonia's X11 backend, directly on the X11 lane and through XWayland on the Wayland lane. On Windows it is driven through UIA. This spec covers its build, its lanes, and the locator rules that hold on both bridges. It also covers the proof that PlatynUI shows an Avalonia tree with a single application level.

## ADDED Requirements

### Requirement: Blueprint catalog from Avalonia standard controls

The fixture SHALL implement the blueprint's core tier and the extended tier's slider, progress bar and tab control, under the canonical names and with the blueprint's name-based observables (capability `test-app-blueprint`). It SHALL build them only from controls that the Avalonia packages it depends on provide, and SHALL NOT draw or implement controls of its own. The extended tier's table SHALL be absent, because it would need a separate package, and the fixture README SHALL say so. The fixture SHALL NOT implement the custom-controls chapter.

#### Scenario: Core tier enumerable through AT-SPI

- **GIVEN** the fixture runs idle in the X11 lane's session or in the Wayland lane's session
- **WHEN** the tree below its application node is walked
- **THEN** every core-tier control that exists while the fixture is idle is found under its canonical name
- **AND** no interactive control reports an empty or duplicate name
- **NOTE** Verifiable only against the real AT-SPI provider. Some core-tier items do not exist while the fixture is idle:
  - the main window is matched by its title (see *Windows are matched by their titles*);
  - menu, context-menu and combo-box items exist only while their popup is open;
  - tree items exist only below an expanded node;
  - `dialog-modal` exists only in an instance started with `--open-modal`.

#### Scenario: Core tier enumerable through UIA on Windows

- **GIVEN** the fixture runs idle on the Windows desktop
- **WHEN** the tree below its application node is walked
- **THEN** every core-tier control that exists while the fixture is idle is found under its canonical name
- **AND** no interactive control reports an empty or duplicate name
- **NOTE** Verifiable only on a real Windows machine. The exceptions are those of the AT-SPI scenario.

#### Scenario: Extended tier without the table

- **GIVEN** the fixture is running
- **WHEN** the extended-tier controls are looked up
- **THEN** `slider-basic`, `progress-basic` and `tabs-basic` with `tab-one` and `tab-two` are found
- **AND** no `table-basic` exists, and the README names the table as not implemented

#### Scenario: A menu activation is observable on every lane

- **GIVEN** the fixture runs on the X11 lane, the Wayland lane or the Windows lane
- **WHEN** `menu-file-new` is activated through the real menu
- **THEN** an element named `last-action-menu-file-new` is resolvable without reopening the menu
- **NOTE** Verifiable only against a real provider.

### Requirement: Windows are matched by their titles

The fixture SHALL use the blueprint's window-naming fallback, because an Avalonia window reports its title as its accessible name. The main window SHALL carry the title given by `--title`, and the catalog suite SHALL match it through the launch configuration. Every dialog SHALL carry its canonical name as its title.

#### Scenario: The main window is found by its launch title

- **GIVEN** the fixture is started with `--title "Avalonia Catalog"`
- **WHEN** the catalog suite locates the main window below the fixture's application node
- **THEN** exactly one window named `Avalonia Catalog` is found
- **NOTE** Verifiable only against a real provider, on every lane.

#### Scenario: A dialog is found by its canonical name

- **GIVEN** an instance of the fixture started with `--open-modal`
- **WHEN** `dialog-modal` is looked up below that instance's application node, without any prior interaction
- **THEN** exactly one window named `dialog-modal` is found, containing `dialog-modal-button` and `dialog-modal-label`
- **NOTE** Verifiable only against a real provider, on every lane.

#### Scenario: A click on the modal dialog's button lands inside the dialog

- **GIVEN** an instance of the fixture started with `--open-modal`
- **WHEN** `dialog-modal-button` is clicked at its reported bounds
- **THEN** an element named `last-action-dialog-modal-button` is resolvable
- **NOTE** Verifiable only against a real provider, on every lane.

### Requirement: Fixture CLI

The fixture SHALL support `--title <text>` with the default `PlatynUI Avalonia TestApp`, `--auto-close <seconds>` and `--open-modal`, as the blueprint defines them. An unknown argument SHALL make it print a usage message naming the argument and exit with a non-zero code before any window opens.

#### Scenario: Title and auto-close are honored

- **WHEN** the fixture starts with `--title "Catalog Fixture" --auto-close 5`
- **THEN** its main window is titled `Catalog Fixture`
- **AND** the process exits with code 0 within a few seconds after the 5-second deadline

#### Scenario: An unknown argument fails fast

- **WHEN** the fixture starts with `--bogus`
- **THEN** it prints a usage message naming `--bogus`, opens no window and exits with a non-zero code

### Requirement: Reproducible build with the current .NET SDK

The fixture SHALL build with the .NET 10 SDK and with package versions that are pinned and locked. `just build-test-app-avalonia` SHALL build it to a fixed path, with the SDK selection pinned to the .NET 10 feature bands. The build SHALL fail instead of building with an SDK outside that pin:

- A machine without a `dotnet` command SHALL get a message naming the .NET 10 SDK.
- A machine whose SDKs the pin does not admit SHALL get the SDK's message naming the requested version.

The lanes SHALL launch the built executable directly, so that the process they start is the fixture's own process. The fixture SHALL stay outside the Cargo workspace.

#### Scenario: The build produces the executable at its fixed path

- **GIVEN** a machine with the .NET 10 SDK
- **WHEN** `just build-test-app-avalonia` runs
- **THEN** the fixture's executable exists at the fixed path the lanes hand over

#### Scenario: The started process is the fixture's process

- **GIVEN** a lane starts the built executable and records the process ID it started
- **WHEN** it selects `/app:Application[@ProcessId=<that id>]`
- **THEN** exactly one application node is selected, and its window is the fixture's main window
- **NOTE** Verifiable only against a real provider.

#### Scenario: A missing dotnet command fails the build

- **GIVEN** a `PATH` without a `dotnet` command
- **WHEN** `just build-test-app-avalonia` runs
- **THEN** it fails with a message naming the .NET 10 SDK

#### Scenario: An SDK outside the pin fails the build

- **GIVEN** a `dotnet` whose installed SDKs the fixture's SDK pin does not admit
- **WHEN** `just build-test-app-avalonia` runs
- **THEN** it fails with a message naming the requested SDK version, and builds nothing

#### Scenario: The Cargo workspace does not see the fixture

- **WHEN** the Cargo workspace is resolved
- **THEN** `apps/test-app-avalonia` is not a workspace member
- **AND** `just check`, `just test` and the mock lane do not need a .NET SDK

### Requirement: Lanes on X11, on Wayland through XWayland, and on Windows

The fixture's acceptance suites SHALL carry no platform tag, so that the X11 lane, the Wayland lane and the Windows lane all select them. On Linux the fixture SHALL run on Avalonia's X11 backend and SHALL NOT use Avalonia's native Wayland backend, because that backend exposes no accessibility. On the Wayland lane it therefore runs through the XWayland of the lane's session, as it does on a Wayland desktop.

- Both Linux sessions SHALL build the fixture before any suite runs and hand its path over as `PLATYNUI_TEST_APP_AVALONIA_BIN`.
- A failed build SHALL end the session with a message naming the .NET 10 SDK and `just build-test-app-avalonia`, before any suite runs.
- The Windows lane SHALL build it as a prerequisite before any suite runs.
- Without the hand-over, for example in a run inside an already established session, the suites SHALL use the path `just build-test-app-avalonia` builds to.
- A suite that is selected while the fixture path it resolves names no executable SHALL fail with a message naming `just build-test-app-avalonia`, and SHALL NOT skip.
- On Linux, a suite that is selected while no X display is available SHALL fail with a message saying that the fixture needs an X display, and that a compositor session provides one only when started with `--xwayland`. It SHALL NOT skip.

#### Scenario: Every lane selects the fixture's suites

- **WHEN** `robotcode discover tests` runs with the profile `real-x11`, `real-wayland` or `real-windows`
- **THEN** each of them lists the tests of `tests/acceptance/avalonia`

#### Scenario: The X11 lane runs the fixture's suites

- **WHEN** `just test-acceptance-x11` runs
- **THEN** the fixture's suites execute inside the X11 session, against a fixture that session built
- **NOTE** Verifiable only on the real X11 lane.

#### Scenario: The Wayland lane runs the fixture through XWayland

- **WHEN** `just test-acceptance-compositor` runs
- **THEN** the fixture's suites execute inside the compositor session, against a fixture that session built
- **AND** the fixture's window is served through the session's XWayland, not over the Wayland protocol
- **NOTE** Verifiable only on the real Wayland lane, for example from the display connections the fixture's process holds.

#### Scenario: A failing fixture build ends a Linux session before the suites

- **GIVEN** a `dotnet` on `PATH` that cannot build the fixture, for example one without a .NET 10 SDK
- **WHEN** `just test-acceptance-x11` or `just test-acceptance-compositor` runs
- **THEN** the run ends with a non-zero exit code and a message naming the .NET 10 SDK and `just build-test-app-avalonia`
- **AND** no suite has run

#### Scenario: A missing fixture fails with guidance

- **GIVEN** the fixture path the launcher resolves names no existing executable
- **WHEN** a fixture suite's setup runs
- **THEN** the suite fails, not skips, with a message naming `just build-test-app-avalonia`

#### Scenario: A compositor session without XWayland fails with guidance

- **GIVEN** an interactive session opened with `scripts/startcompositor.sh` without `--xwayland`
- **WHEN** a fixture suite runs inside it with the `real-wayland` profile
- **THEN** the suite fails, not skips, with a message saying that the fixture needs an X display and naming `scripts/startcompositor.sh --xwayland`
- **NOTE** Verifiable only in a real compositor session.

### Requirement: Locators hold on AT-SPI and UIA

The catalog suite SHALL address catalog controls by their names alone, relative to the fixture's application node, and SHALL use role unions only to find windows. It SHALL NOT contain per-platform variants of a locator.

A difference between the bridges or the lanes that name-only locators cannot absorb SHALL be verified on each affected lane and documented in the fixture README. The affected test SHALL then be handled by the first rule that fits:

- **Behavior that exists on exactly one lane** is scoped to that lane by its platform tag. A platform tag confines a test to exactly one lane, because each lane excludes every foreign platform tag.
- **A limitation of the toolkit that holds on every lane** stays as the blueprint's documented skip.
- **A difference that holds on some lanes but not all** keeps the test on every lane. The test asserts only the facts that hold on all of them, as the blueprint's documented-deviation rule describes.

A defect of PlatynUI SHALL NOT be handled by any of these rules. It is fixed in PlatynUI, and until then the affected test fails.

#### Scenario: The same catalog test passes unchanged on every lane

- **GIVEN** the catalog suite's test for `list-item-3`
- **WHEN** it runs on the X11 lane, the Wayland lane and the Windows lane
- **THEN** it passes on each with the same locator, although the list entry has a different role on AT-SPI than on UIA
- **NOTE** Verifiable only against real providers.

#### Scenario: A popup's items are found from the application node

- **GIVEN** the context menu `context-menu` is open, which Avalonia shows in a window of its own
- **WHEN** `ctx-copy` is looked up by name below the fixture's application node
- **THEN** it is found on every lane and can be activated by a pointer click at its reported bounds
- **AND** `last-action-ctx-copy` appears
- **NOTE** Verifiable only against real providers.

#### Scenario: A difference on some lanes narrows the assertion, not the selection

- **GIVEN** a catalog behavior that the UIA bridge provably cannot show while both Linux lanes show it
- **WHEN** the catalog suite runs on each lane
- **THEN** the affected test runs on all three lanes and asserts only the facts that hold on all of them
- **AND** the README documents the difference and the facts the test leaves out

#### Scenario: A limitation on every lane is a documented skip

- **GIVEN** a catalog behavior that Avalonia provably cannot show on any lane
- **WHEN** the catalog suite runs
- **THEN** the affected test is skipped with a message naming the limitation and the README section that documents it

### Requirement: One application level on a real Avalonia tree

The fixture's suites SHALL show on every lane that PlatynUI presents the fixture with a single application level: its main window is a window below exactly one application node, and the window's content is not treated as a window.

#### Scenario: The main window hangs below exactly one application node

- **GIVEN** the fixture is running
- **WHEN** the ancestors of `button-basic` are read
- **THEN** exactly one of them is in the `app` namespace, and the main window is its direct child
- **NOTE** Verifiable only against real providers, on every lane.

#### Scenario: Only the window is a window

- **GIVEN** the fixture is running
- **WHEN** the supported patterns of the main window and of `button-basic` are read
- **THEN** the main window exposes `Activatable`, and `button-basic` does not
- **NOTE** Verifiable only against real providers, on every lane.
