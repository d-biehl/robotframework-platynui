# Spec Delta

## ADDED Requirements

### Requirement: Name-source coverage

The app SHALL contain a titled panel with the accessible name `names-panel` that carries the fixture's coverage of where a name comes from. It holds:

- a push button with the accessible name `names-button`, the component name `namesButton` (set through `setName`), the accessible description `A button with a developer name`, and a visible label that differs from all three;
- a read-only table with the accessible name `names-table`, one row and two columns:
  - `amount` holds the `Double` `1234.5` and displays it as `1,234.50`;
  - `active` holds the `Boolean` `true` and displays it as a check box without text.

The displayed text SHALL NOT depend on the JVM's default locale. Adding the panel SHALL NOT change any existing accessible name, and `main-table` SHALL keep its 100×6 shape and its preselected row.

#### Scenario: The named button carries two different names

- **WHEN** the fixture's tree is read through the Java agent
- **THEN** the button reports the accessible name `names-button`, the component name `namesButton` and the accessible description `A button with a developer name`
- *(Real provider only; the Java agent on Windows.)*

#### Scenario: The table's displayed text differs from its model values

- **WHEN** the cells of `names-table` are read through the Java agent
- **THEN** the `amount` cell displays `1,234.50` and holds the model value `1234.5`
- **AND** the `active` cell displays no text and holds the model value `true`
- *(Real provider only.)*

#### Scenario: The displayed amount does not follow the default locale

- **GIVEN** the fixture launched with a German default locale (`-Duser.language=de -Duser.country=DE`)
- **WHEN** the `amount` cell of `names-table` is read through the Java agent
- **THEN** it displays `1,234.50`, not `1.234,50`
- *(Real provider only.)*

#### Scenario: Existing names and the main table stay as they were

- **WHEN** the fixture with the new panel is read through the Java Access Bridge
- **THEN** every accessible name the fixture designated before is present unchanged, and `main-table` still has 100 rows and 6 columns with row 2 selected
- *(Real provider only; the existing JAB suites guard it.)*
