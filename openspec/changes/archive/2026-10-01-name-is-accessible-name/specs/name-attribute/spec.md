# Spec Delta

## Purpose

The common `control:Name` attribute and the node's name accessor. It covers what they carry on each provider this capability names, that the value is the accessible name the toolkit reports and nothing else, that it may be empty, and where the values that are not the name stay readable instead.

## ADDED Requirements

### Requirement: Name is the accessible name the toolkit reports, taken by source

The system SHALL define `Name` as a common attribute in the `control` namespace (canonical constant `attribute_names::common::NAME`) on every `control:` and `item:` node. Its value SHALL be the accessible name the element's toolkit reports. Each provider takes it from one source:

- UI Automation: `CurrentName`.
- AT-SPI: `Accessible.Name`.
- The Java Access Bridge: the accessible name the bridge reports for the element.
- The Java agent's Swing/AWT adapter: the accessible name Swing reports (`AccessibleContext.getAccessibleName()`). For a component or a window this is its own accessible context. For a table cell, a column header or a list entry it is the accessible context Swing provides for that item.

Every provider SHALL take the value as reported, without rewriting it (capability `attribute-values`).

Where the toolkit itself derives the accessible name, the derived value is the accessible name and SHALL be used as reported. Swing, for example, names a frame or dialog after its title and a table cell after the text its renderer displays when nothing else is set.

A provider SHALL NOT substitute another value for a missing or empty accessible name: not the element's identifier (`Id`), an object attribute, a window title, a table model value, a column header value, a class name or the role. This capability does not govern application nodes; their name follows `application-process-attributes`.

#### Scenario: A Swing component's developer name is its Id, not its Name

- **GIVEN** the Swing test application served by the Java agent, with the push button whose accessible name is `names-button` and whose component name is `namesButton`
- **WHEN** the button's `@Name`, its name accessor, its `@Id` and its `native:ComponentName` are read
- **THEN** `@Name` and the name accessor SHALL be `names-button`, and `@Id` and `native:ComponentName` SHALL be `namesButton`
- **AND** `//control:Button[@Name="namesButton"]` SHALL select nothing
- **NOTE** Real provider only; the Java agent on the Windows lane.

#### Scenario: The same button has the same Name on both Java backends

- **GIVEN** the Swing test application, once served through the Java Access Bridge and once through the Java agent
- **WHEN** the push button whose accessible name is `names-button` and whose component name is `namesButton` is located by `@Name="names-button"` on each backend
- **THEN** each backend SHALL find exactly that button
- **NOTE** Real provider only; the Java live fixture on Windows.

#### Scenario: A window is named by its accessible name, not by its title

- **GIVEN** the Swing test application launched in its second-`AppContext` mode with its companion window shown, whose title is `<title> companion` and whose accessible name is `companion-window`
- **WHEN** the companion window is read through the Java agent
- **THEN** its `@Name` SHALL be `companion-window`, and its title SHALL remain readable as `native:WindowTitle` with the value `<title> companion`
- **NOTE** Real provider only; the Java agent on Windows.

#### Scenario: A window without an explicit accessible name is named after its title by Swing

- **GIVEN** the Swing test application's main frame, whose title is `T` and which carries no explicitly set accessible name
- **WHEN** the frame is read through the Java agent
- **THEN** its `@Name` SHALL be `T`
- **NOTE** Real provider only. This holds because Swing reports the title as the frame's accessible name, not because the provider reads the title.

#### Scenario: A table cell is named by the text it displays

- **GIVEN** the Swing test application's `names-table`, whose `amount` cell holds the model value `1234.5` and displays `1,234.50`
- **WHEN** the cell is read through the Java agent
- **THEN** its `@Name` SHALL be `1,234.50`, and no `@Name="1234.5"` SHALL select it
- **NOTE** Real provider only.

#### Scenario: A table cell whose renderer displays no text has an empty Name

- **GIVEN** the Swing test application's `names-table`, whose `active` cell holds the model value `true` and is displayed as a check box without text
- **WHEN** the cell is read through the Java agent
- **THEN** its `@Name` SHALL be the empty string, not `true`
- **NOTE** Real provider only.

#### Scenario: A column header is named by its accessible name

- **GIVEN** the Swing test application's `names-table`, whose first column header displays `amount`
- **WHEN** the header is read through the Java agent
- **THEN** its `@Name` SHALL be `amount`
- **NOTE** Real provider only.

#### Scenario: A list entry is named by its accessible name

- **GIVEN** a Swing list whose renderer displays `Alpha` for its first entry
- **WHEN** the entry is described by the Java agent
- **THEN** its name SHALL be `Alpha`
- **NOTE** Verified on the agent's description in isolation; the fixture has no list outside a combo box popup.

#### Scenario: An element is named by its accessible name on every bridge

- **GIVEN** the egui test application, whose Click Me button reports the accessible name `Click Me`
- **WHEN** the button's `@Name` and its name accessor are read
- **THEN** both SHALL be `Click Me`
- **NOTE** Real provider only. Runs in the X11, compositor and Windows lanes, so it covers AT-SPI and UI Automation. It holds today, and the scenario guards it.

#### Scenario: An empty Accessible.Name is no name, whatever the object attributes carry

- **GIVEN** an AT-SPI element whose `Accessible.Name` is empty, and whose object attributes carry `accessible-name`, `name`, `label` and `title`
- **WHEN** its `Name` is decided
- **THEN** it SHALL be the empty string
- **NOTE** Verified on the provider's decision in isolation. Until this change, AT-SPI takes the first of those attributes.

### Requirement: Name is always present, may be empty, and the attribute and the accessor agree

A provider SHALL list `control:Name` on every `control:` and `item:` node. When the toolkit reports no accessible name, or an empty one, the value SHALL be the empty string, never absent. The node's name accessor (`element.name` in Python) SHALL return the same value as the attribute. Enumerating a node's attributes and looking `Name` up by name SHALL agree.

#### Scenario: A component with a developer name but no accessible name has an empty Name

- **GIVEN** a Swing window served by the Java agent, whose layered pane Swing named `null.layeredPane` and gave no accessible name
- **WHEN** the layered pane's attributes are enumerated, `Name` is looked up by name, and its name and id accessors are read
- **THEN** `control:Name` SHALL be listed with the empty string, the lookup and the name accessor SHALL return the empty string, and `@Id` and the id accessor SHALL be `null.layeredPane`
- **NOTE** Real provider only; the Java agent on Windows.

#### Scenario: An empty Name is matched as empty

- **GIVEN** the same layered pane
- **WHEN** `//*[@Id="null.layeredPane"][@Name=""]` is evaluated below its window
- **THEN** it SHALL select the layered pane
- **NOTE** Real provider only.

### Requirement: Values that are not the name stay readable

A provider SHALL keep the values it no longer reports as `Name` readable under attributes of their own.

The Java agent keeps:

- a component's or window's component name as `control:Id` (spec `id-attribute`) and as `native:ComponentName`;
- a window's title as `native:WindowTitle`;
- a table cell's model value as `native:TableCell.ModelValue`.

AT-SPI keeps an element's object attributes, unchanged, in `native:Accessible.Attributes`.

`native:ComponentName` SHALL be listed only on components and windows whose component name was set. A table cell, a column header, a table row or a list entry SHALL NOT list it.

`native:TableCell.ModelValue` SHALL carry the model value typed:
- an integral number (byte, short, int, long) as an integer;
- a floating-point number as a number, except that a value that is not finite (NaN or an infinity) is its string form;
- a boolean as a boolean;
- any other value as its string form.

A cell whose model value is null SHALL NOT list `native:TableCell.ModelValue`.

#### Scenario: A formatted cell keeps its model value as a number

- **GIVEN** the Swing test application's `names-table`, whose `amount` cell holds `1234.5` and displays `1,234.50`
- **WHEN** the cell's `native:TableCell.ModelValue` is read through the Java agent
- **THEN** it SHALL be the number `1234.5`
- **NOTE** Real provider only.

#### Scenario: A boolean cell keeps its model value as a boolean

- **GIVEN** the Swing test application's `names-table`, whose `active` cell holds `true`
- **WHEN** the cell's `native:TableCell.ModelValue` is read through the Java agent
- **THEN** it SHALL be the boolean `true`
- **NOTE** Real provider only.

#### Scenario: A string cell keeps its model value as a string

- **GIVEN** the Swing test application's `main-table`, whose cell in row 2, column 0 holds `r2c0`
- **WHEN** the cell's `native:TableCell.ModelValue` is read through the Java agent
- **THEN** it SHALL be the string `r2c0`
- **NOTE** Real provider only.

#### Scenario: A model value that is not a finite number keeps its string form

- **GIVEN** a Swing table cell whose model value is the double NaN
- **WHEN** the cell is described by the Java agent and mapped by the provider
- **THEN** `native:TableCell.ModelValue` SHALL be the string `NaN`, not absent and not null
- **NOTE** Verified on the agent's description and the provider's mapping in isolation.

#### Scenario: A null model value lists no model value

- **GIVEN** a Swing table cell whose model value is null
- **WHEN** the cell is described by the Java agent
- **THEN** it SHALL carry no model value, and its node SHALL list no `native:TableCell.ModelValue`
- **NOTE** Verified on the agent's description and the provider's mapping in isolation; the fixture has no null cell.

#### Scenario: AT-SPI object attributes stay readable

- **GIVEN** an AT-SPI element whose object attributes carry `label` with the value `Save`
- **WHEN** its native attributes are mapped
- **THEN** `native:Accessible.Attributes` SHALL carry `label` with the value `Save`
- **NOTE** Verified on the provider's mapping in isolation. It holds today, and the scenario guards it.

#### Scenario: A cell lists no component name

- **GIVEN** any table cell served by the Java agent
- **WHEN** its attributes are enumerated
- **THEN** no `native:ComponentName` SHALL be listed
- **NOTE** Real provider only. Today a cell lists its model value as `native:ComponentName`.
