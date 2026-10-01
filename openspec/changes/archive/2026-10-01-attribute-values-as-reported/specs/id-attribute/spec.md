# Spec Delta

## MODIFIED Requirements

### Requirement: Id is the identifier the toolkit reports, taken by source

The system SHALL define `Id` as a common attribute in the `control` namespace (canonical constant `attribute_names::common::ID`). Its value SHALL be the identifier the element's toolkit reports for automation. Each provider takes it from one source:

- UI Automation: `AutomationId`.
- AT-SPI: `Accessible.AccessibleId`; when that is the empty string or cannot be read, the first of the object attributes `accessible-id`, `accessible_id` and `id` that is not the empty string.
- The Java agent's Swing/AWT adapter: `Component.getName()` when the name was set through `setName`, for components and windows only; table cells, rows and other accessibility-only children have none.
- macOS Accessibility: `AXIdentifier`, once that provider lists elements.
- The Java Access Bridge and the mock provider report no identifier.

A toolkit that forwards an application's automation id through its accessibility API (AccessKit's author id reaches UIA and AT-SPI) SHALL surface it through that source. The provider SHALL take the value by source, whoever set it: an identifier the toolkit or framework generated counts as the element's `Id` just like one the application set. The provider SHALL NOT substitute another value for a missing identifier: not the name, the process ID, the program name or the runtime id.

Every provider SHALL take the identifier unmodified, whichever source it came from (capability `attribute-values`). An identifier that consists only of whitespace is an identifier, not an empty one.

#### Scenario: An AccessKit author id is the Id on every bridge

- **GIVEN** the egui test application, whose Click Me button carries the AccessKit author id `btn-click-me`
- **WHEN** the button's `@Id` and its id accessor (`${el.id}`) are read
- **THEN** both SHALL be `btn-click-me`
- **NOTE** Real provider only. Runs in the X11, compositor and Windows lanes, so it covers AT-SPI and UI Automation.

#### Scenario: An identifier Swing sets itself counts as the Id

- **GIVEN** a Swing application served by the Java agent, with a `JSpinner` whose increment button Swing named `Spinner.nextButton`
- **WHEN** that button's `@Id` is read
- **THEN** it SHALL be `Spinner.nextButton`
- **NOTE** Real provider only; the Java agent's live fixture on Windows.

#### Scenario: A table cell has no Id although the agent reports a name for it

- **GIVEN** a Swing table served by the Java agent, whose cell reports its model value as its name
- **WHEN** the cell's attributes and id accessor are read
- **THEN** it SHALL carry no `control:Id`, and its id accessor SHALL return none
- **NOTE** Real provider only; the Java agent's live fixture on Windows.

#### Scenario: An AT-SPI identifier keeps its whitespace

- **GIVEN** an AT-SPI element whose `Accessible.AccessibleId` is ` btn-save `
- **WHEN** its `Id` is decided
- **THEN** it SHALL be ` btn-save `, unchanged
- **NOTE** Verified on the provider's decision in isolation. Today AT-SPI trims it to `btn-save`.

#### Scenario: A whitespace-only AT-SPI identifier is not replaced by an object attribute

- **GIVEN** an AT-SPI element whose `Accessible.AccessibleId` consists of two spaces, and whose object attribute `id` is `btn-save`
- **WHEN** its `Id` is decided
- **THEN** it SHALL be the two spaces
- **NOTE** Verified on the provider's decision in isolation. Today AT-SPI treats the blank value as missing and takes `btn-save`.

#### Scenario: An AT-SPI object-attribute identifier is taken unmodified

- **GIVEN** an AT-SPI element whose `Accessible.AccessibleId` is the empty string, whose object attribute `accessible-id` is the empty string, and whose object attribute `id` is ` btn-save `
- **WHEN** its `Id` is decided
- **THEN** it SHALL be ` btn-save `, taken from `id` and unchanged
- **NOTE** Verified on the provider's decision in isolation.
