# gtk-test-app Specification

## Purpose

Defines the GTK 3 and GTK 4 fixture apps: blueprint-conforming catalogs built from each version's standard widgets, the window shape that geometry tests rely on, how the lanes start them, the GTK 4 versions it supports, and the names of GTK 4's popover menu items.

## Requirements

### Requirement: One blueprint fixture per GTK major version

There SHALL be one fixture for GTK 3 and one for GTK 4. Each SHALL implement the blueprint's core tier and the extended tier's slider, progress bar and tab control under the canonical names, from that version's standard widgets. GTK 4 SHALL use widgets not deprecated in 4.10, except where a current widget provably does not expose a required name or state, which its README documents.

#### Scenario: The GTK 3 core tier is enumerable through AT-SPI

- **GIVEN** the GTK 3 fixture running on either Linux lane
- **WHEN** the tree under its main window is walked
- **THEN** every core-tier control and the slider, progress bar and tab control are found by their canonical names, and no interactive control has an empty or duplicate name
- **NOTE** Real provider only.

#### Scenario: The GTK 4 core tier is enumerable through AT-SPI

- **GIVEN** the GTK 4 fixture running on either Linux lane
- **WHEN** the tree under its main window is walked
- **THEN** every core-tier control and the slider, progress bar and tab control are found by their canonical names, and no interactive control has an empty or duplicate name
- **NOTE** Real provider only.

#### Scenario: A deprecated widget is a documented choice

- **GIVEN** a GTK 4 catalog control built from a widget deprecated in 4.10
- **WHEN** the fixture's README is read
- **THEN** it names the control, the current widget that was tried and what it failed to expose in the real tree
- **NOTE** Review check at implementation; no automated test.

### Requirement: Windows are matched by their titles

Each fixture's main window SHALL be found by its launch title, and each dialog SHALL carry its canonical name as its title, following the blueprint's window-naming fallback. A fixture SHALL NOT rely on an accessible name that differs from the window title.

#### Scenario: The main window is found by its launch title

- **GIVEN** a GTK fixture started with `--title "GTK Catalog"`
- **WHEN** its window is looked up below its application node by the name `GTK Catalog`
- **THEN** exactly one window is found
- **NOTE** Real provider only, both Linux lanes.

#### Scenario: A dialog is found by its canonical name

- **GIVEN** a GTK fixture whose modeless dialog has been opened
- **WHEN** a window named `dialog-modeless` is looked up below its application node
- **THEN** the dialog is found, and a click on `dialog-modeless-button` makes `last-action-dialog-modeless-button` resolvable
- **NOTE** Real provider only, both Linux lanes.

### Requirement: The window shape geometry tests rely on

Each fixture's main window SHALL use a header bar as its title bar, so the toolkit draws its own decorations. The modeless dialog SHALL open with a size different from the main window's. `groupbox-basic` SHALL contain `radio-first` and `radio-second` in the tree.

#### Scenario: The header bar is part of the main window

- **GIVEN** a GTK fixture running under the PlatynUI compositor
- **WHEN** the compositor's window list is read for the fixture's main window
- **THEN** it reports client-side decorations for that window
- **AND** the window's title is found in the tree below the main window, in its header bar
- **NOTE** Real provider only, compositor lane.

#### Scenario: The modeless dialog differs in size from the main window

- **GIVEN** a GTK fixture with its modeless dialog open
- **WHEN** both windows' `Bounds` are read
- **THEN** their widths or their heights differ by more than the X11 lookup's size tolerance
- **NOTE** Real provider only, both Linux lanes.

#### Scenario: The radio buttons are inside the group

- **GIVEN** a GTK fixture running
- **WHEN** `radio-first` is looked up relative to `groupbox-basic`
- **THEN** it is found as a descendant of the group, not beside it
- **NOTE** Real provider only, both Linux lanes.

### Requirement: Fixture CLI

Each fixture SHALL accept `--title <text>` (defaults `PlatynUI GTK3 TestApp` and `PlatynUI GTK4 TestApp`), `--auto-close <seconds>`, `--open-modal` and `--app-id <id>`. An unknown argument SHALL fail with a usage message and a non-zero exit code before any window opens.

#### Scenario: Title and auto-close are honored

- **WHEN** a GTK fixture starts with `--title "Catalog Fixture" --auto-close 5`
- **THEN** its main window's title is `Catalog Fixture`, and the process exits with code 0 within a few seconds after the five-second deadline
- **NOTE** Real provider only, both Linux lanes.

#### Scenario: Modal state is reachable without interaction

- **WHEN** a GTK fixture starts with `--open-modal`
- **THEN** `dialog-modal` and its contents are in the tree without any pointer or keyboard input
- **NOTE** Real provider only; `modal.robot` of each fixture.

#### Scenario: An unknown argument fails fast

- **WHEN** a GTK fixture starts with `--bogus`
- **THEN** it prints a usage message naming `--bogus` and exits with a non-zero code, and no window appears on the accessibility bus
- **NOTE** A test in each catalog suite starts the fixture with `--bogus` and checks its exit code and output.

### Requirement: The fixtures run on the project environment's PyGObject

The fixtures SHALL run on the project environment's interpreter, whose dev dependencies include PyGObject on Linux. The suites SHALL launch that interpreter directly, so the launched process is the fixture's process. When PyGObject or a GTK version's introspection data is missing, the fixture SHALL exit non-zero with a message naming what is missing and the package that provides it.

#### Scenario: The launched process is the fixture's process

- **GIVEN** a GTK fixture launched by its suite
- **WHEN** its window is looked up below `/app:Application[@ProcessId=<launched pid>]`
- **THEN** the window is found
- **NOTE** Real provider only, both Linux lanes.

#### Scenario: Missing introspection data fails with the package name

- **GIVEN** an environment where PyGObject is installed but GTK 4's introspection data is not
- **WHEN** the GTK 4 fixture starts
- **THEN** it exits non-zero with a message naming GTK 4's introspection data and the package `gir1.2-gtk-4.0`
- **AND** the suite's setup fails with that message, not a skip
- **NOTE** Run once by hand at implementation with `GI_TYPELIB_PATH` pointing to an empty directory; the lanes always have the data.

### Requirement: The fixtures run on both Linux lanes and not on Windows

The GTK suites SHALL carry `platform:linux` and run on the X11 lane and under the PlatynUI compositor, where both GTK versions use their native Wayland backend. Each Linux session SHALL hand the fixtures over through `PLATYNUI_TEST_APP_GTK3_PYTHON`/`_MAIN` and `PLATYNUI_TEST_APP_GTK4_PYTHON`/`_MAIN`.

#### Scenario: Both Linux lanes select the GTK suites

- **WHEN** `robotcode --profile real-x11 discover tests` and the same with `real-wayland` run
- **THEN** both list the GTK 3 and GTK 4 catalog and modal suites
- **NOTE** Discovery only; no session needed.

#### Scenario: The Windows lane does not select them

- **WHEN** `robotcode --profile real-windows discover tests` runs
- **THEN** no GTK suite is listed
- **NOTE** Discovery only.

#### Scenario: The GTK fixtures run natively on Wayland

- **GIVEN** the compositor lane
- **WHEN** a GTK fixture's window is read from the compositor's window list
- **THEN** it is listed as a Wayland client with the fixture's process ID
- **NOTE** Real provider only, compositor lane.

### Requirement: The GTK 4 fixture supports GTK 4.18 and later

The GTK 4 fixture and its suites SHALL target GTK 4.18 or later, and the CI runner SHALL provide such a GTK 4. Older GTK 4 releases are not supported.

#### Scenario: A menu test runs on a supported GTK 4

- **GIVEN** GTK 4.18 or later
- **WHEN** the GTK 4 catalog's menu-bar test runs
- **THEN** activating `menu-file-new` makes `last-action-menu-file-new` resolvable
- **NOTE** Real provider only, both Linux lanes. Passes once `linux-element-geometry` places popovers where the platform shows them.

#### Scenario: CI runs a supported GTK 4

- **GIVEN** the CI runner image
- **WHEN** the acceptance job runs a Linux lane
- **THEN** its log shows GTK 4.18 or later, and the GTK 4 popover tests run with the other GTK tests
- **NOTE** Checked in the CI run's log and results.

### Requirement: The GTK 4 fixture names its popover menu items

The GTK 4 fixture SHALL give every menu-bar and context-menu item its label as its accessible name, the items of submenus included, and the context menu's popover the name `context-menu`. The catalog SHALL find each item by its canonical `@Name` below the named menu it belongs to, also on GTK 4.18 to 4.24.1, which leave the items unnamed.

#### Scenario: A submenu item is found by its name

- **GIVEN** GTK 4.24.1 and the GTK 4 fixture with `menu-edit` open and its submenu `menu-edit-more` open
- **WHEN** `menu-edit-sub-one` is looked up by `@Name` below `menu-edit` and `menu-edit-more`
- **THEN** it SHALL be found
- **NOTE** Real provider only, both Linux lanes; the catalog's submenu test, whose locators resolve before each click.

#### Scenario: A context-submenu item is found by its name

- **GIVEN** GTK 4.24.1 and the GTK 4 fixture with its context menu open and the submenu `ctx-more` slid in
- **WHEN** `ctx-sub-alpha` is looked up by `@Name` below `context-menu`
- **THEN** it SHALL be found
- **NOTE** Real provider only, both Linux lanes; the catalog's context-submenu test.
