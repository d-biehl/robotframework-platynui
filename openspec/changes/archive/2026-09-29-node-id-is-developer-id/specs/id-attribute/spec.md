# Spec Delta

## Purpose

The common `control:Id` attribute and the node's id accessor: which identifier they carry on each provider, when they are present, that the two always agree, and that application nodes carry none, because an application is identified by its process ID.

## ADDED Requirements

### Requirement: Id is the identifier the toolkit reports, taken by source

The system SHALL define `Id` as a common attribute in the `control` namespace (canonical constant `attribute_names::common::ID`). Its value SHALL be the identifier the element's toolkit reports for automation. Each provider takes it from one source:

- UI Automation: `AutomationId`.
- AT-SPI: `Accessible.AccessibleId`; when that is empty or cannot be read, the object attribute `accessible-id`, `accessible_id` or `id`.
- The Java agent's Swing/AWT adapter: `Component.getName()` when the name was set through `setName`, for components and windows only; table cells, rows and other accessibility-only children have none.
- macOS Accessibility: `AXIdentifier`, once that provider lists elements.
- The Java Access Bridge and the mock provider report no identifier.

A toolkit that forwards an application's automation id through its accessibility API (AccessKit's author id reaches UIA and AT-SPI) SHALL surface it through that source. The provider SHALL take the value by source, whoever set it: an identifier the toolkit or framework generated counts as the element's `Id` just like one the application set. The provider SHALL NOT substitute another value for a missing identifier: not the name, the process ID, the program name or the runtime id.

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

### Requirement: Id is present only with a value, and the attribute and the accessor agree

A provider SHALL list `control:Id` exactly when the element has an identifier, and SHALL NOT list it with an empty value. The node's id accessor (`element.id` in Python) SHALL return the same value as the attribute, and none exactly when the attribute is absent. Enumerating a node's attributes and looking `Id` up by name SHALL agree. An identifier that cannot be read — the call fails or does not answer in time — SHALL leave the `Id` absent for that read, not empty.

#### Scenario: An element without an identifier has no Id

- **GIVEN** an element of the egui test application whose toolkit reports no identifier
- **WHEN** its attributes are enumerated, `Id` is looked up by name and its id accessor is read
- **THEN** no `control:Id` SHALL be listed, the lookup SHALL find none, `[@Id]` SHALL not match it, and the accessor SHALL return none
- **NOTE** Real provider only. Runs in the X11, compositor and Windows lanes. Today AT-SPI lists `@Id=""` for such an element.

#### Scenario: An empty accessible-id is no Id

- **GIVEN** an AT-SPI element whose `Accessible.AccessibleId` is the empty string and whose object attributes carry no identifier
- **WHEN** its `Id` is decided
- **THEN** it SHALL carry no `control:Id`, and its id accessor SHALL return none
- **NOTE** Verified on the provider's decision in isolation.

#### Scenario: Enumeration and lookup agree

- **GIVEN** an element with an identifier and an element without one
- **WHEN** each element's `Id` is read by enumerating its attributes (`@*`), by looking it up by name (`@Id`) and through its id accessor
- **THEN** all three reads SHALL return the identifier for the first element, and none SHALL find one for the second
- **NOTE** Real provider only. The egui acceptance cases cover AT-SPI and UI Automation on every real lane. The UI Automation unit tests cover a test-window button with a control ID (Windows only), and the Java live fixture covers the agent's and the bridge's window trees (Windows only).

#### Scenario: An identifier that does not answer in time is absent

- **GIVEN** an AT-SPI element whose `Accessible.AccessibleId` read does not answer in time, and whose object-attribute read does not answer either or carries no identifier
- **WHEN** its attributes are read
- **THEN** it SHALL carry no `control:Id` for that read, and in particular not one with the value `""`
- **NOTE** Verified on the provider's decision in isolation for a read that answers nothing. That an unanswered call answers nothing is the read's own behaviour, which this change does not alter.

### Requirement: Application nodes carry no Id

A node at the application level — the `app` namespace, on every provider — SHALL carry no `Id`: no `control:Id` attribute, and its id accessor SHALL return none. This SHALL hold whatever the node's process ID, its name or program name, and any identifier its toolkit root reports. An application's identity SHALL be its process-ID attribute (`@ProcessId`). Its one-line description SHALL therefore carry no `#` suffix. On AT-SPI, an identifier the application's root object reports SHALL remain readable only as the native attribute `native:Accessible.AccessibleId`.

#### Scenario: An application has no Id although its process ID is known

- **GIVEN** the egui test application launched with the process ID `N`
- **WHEN** `/app:Application[@ProcessId=N]` is evaluated, the node's attributes are enumerated, `Id` is looked up by name and its id accessor is read
- **THEN** exactly that application node SHALL be selected, no `control:Id` SHALL be listed, the lookup SHALL find none, and its id accessor SHALL return none
- **NOTE** Real provider only. Runs in the X11, compositor and Windows lanes. On AT-SPI, today the id accessor returns `N`, and `@Id` is listed with the root's accessible-id or `""`. On UI Automation, today the id accessor, the listed `Id` and the lookup by name all return the node's name, its `app:ProcessName`; the listing and the lookup answer it on separate paths.

#### Scenario: No application is selected by Id

- **GIVEN** a desktop with running applications
- **WHEN** `/app:*[@Id]` is evaluated
- **THEN** it SHALL select nothing
- **NOTE** Real provider only. Runs in every real lane.

#### Scenario: An application root's own identifier is not its Id

- **GIVEN** an AT-SPI application whose root object reports the accessible-id `QApplication`, as a Qt application does when it sets no application name
- **WHEN** its application node is read
- **THEN** it SHALL carry no `control:Id`, its id accessor SHALL return none, and `native:Accessible.AccessibleId` SHALL be `QApplication`
- **NOTE** The `Id` half holds by construction: at the application level the decision does not read the accessible-id; a unit test pins it. The native half needs a real Qt application and is checked by hand (shared suites use no `native:` attributes).

#### Scenario: An application without a process ID has no Id either

- **GIVEN** two AT-SPI applications for which the bus daemon cannot tell the process ID
- **WHEN** their id accessors are read
- **THEN** both SHALL return none, and in particular neither SHALL return `0`
- **NOTE** Holds by construction: the decision at the application level takes no process ID; a unit test pins that it answers none.

#### Scenario: Java application nodes have no Id

- **GIVEN** the Swing test application, once through the Java Access Bridge and once through the Java agent
- **WHEN** each application node's attributes and id accessor are read
- **THEN** neither SHALL carry a `control:Id`, both id accessors SHALL return none, and both SHALL carry `@ProcessId`
- **NOTE** Real provider only; the Java live fixture on Windows. Both nodes already behave so; the scenario guards it.

#### Scenario: An application is described without an id

- **GIVEN** the egui test application launched with the process ID `N`
- **WHEN** its application node is described in one line, as action log lines do
- **THEN** the description SHALL end with the quoted name, without a `#` suffix
- **NOTE** Real provider only. Runs in the X11, compositor and Windows lanes. Today AT-SPI appends `#N` and UI Automation the process name.
