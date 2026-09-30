# Spec Delta

## MODIFIED Requirements

### Requirement: Strict per-platform source mapping
Each provider SHALL source `Description` exclusively from its platform's accessible-description property:

- AT-SPI2: `Accessible.Description`;
- Windows UIA: `FullDescription` (`UIA_FullDescriptionPropertyId`);
- the Java Access Bridge: the accessible description the bridge reports for the element;
- the Java agent's Swing/AWT adapter: the accessible description Swing reports (`AccessibleContext.getAccessibleDescription()`).

Providers MUST NOT fall back to help/tooltip-like properties (`Accessible.HelpText`, UIA `HelpText`, `LegacyIAccessible.Description`); those remain available under the `native:` namespace only. Where the toolkit itself derives the accessible description, the derived value is the accessible description. Swing, for example, reports a component's tool tip text as its accessible description when nothing else is set. The macOS AX provider is a stub and emits no `Description`.

Every provider SHALL take the value unmodified (capability `attribute-values`). A description that consists only of whitespace is a description, not an empty one.

#### Scenario: AT-SPI sources Accessible.Description only
- **GIVEN** an AT-SPI element whose `Accessible.Description` is empty but whose `Accessible.HelpText` is non-empty
- **WHEN** the element's attributes are enumerated
- **THEN** no `control:Description` attribute is present
- **AND** `native:Accessible.HelpText` still exposes the help text
- *(real-provider only: the mock does not model AT-SPI HelpText)*

#### Scenario: UIA sources FullDescription only
- **GIVEN** a UIA element whose `FullDescription` property is empty but whose `HelpText` property is non-empty
- **WHEN** the element's attributes are enumerated
- **THEN** no `control:Description` attribute is present
- *(real-provider only: verified manually/via Inspector on Windows; no automated UIA lane exists)*

#### Scenario: UIA attribute() fast-path and attributes() agree
- **GIVEN** a UIA element with a non-empty `FullDescription`
- **WHEN** the attribute is read via direct lookup `attribute(Control, "Description")` and via full enumeration `attributes()`
- **THEN** both return the same value
- *(real-provider only)*

#### Scenario: AT-SPI keeps the description's whitespace
- **GIVEN** an AT-SPI element whose `Accessible.Description` is `  Closes the dialog  `
- **WHEN** its `Description` is decided
- **THEN** it SHALL be `  Closes the dialog  `, unchanged
- *(verified on the provider's decision in isolation)*

#### Scenario: A whitespace-only AT-SPI description is listed as reported
- **GIVEN** an AT-SPI element whose `Accessible.Description` consists of three spaces
- **WHEN** its `Description` is decided
- **THEN** it SHALL be the three spaces, not absent
- *(verified on the provider's decision in isolation)*

#### Scenario: The native AT-SPI description is the description as reported
- **GIVEN** an AT-SPI element whose `Accessible.Description` is `Closes the dialog ` with a trailing space
- **WHEN** its `native:Accessible.Description` is read
- **THEN** it SHALL be `Closes the dialog `, equal to its `control:Description`
- *(verified on the provider's mapping in isolation)*

#### Scenario: The Java Access Bridge sources the bridge's description
- **GIVEN** the Swing test application served through the Java Access Bridge, with the push button `names-button` whose accessible description is `A button with a developer name`
- **WHEN** the button's attributes are enumerated, `Description` is looked up by name, and its description accessor is read
- **THEN** all three SHALL return `A button with a developer name`
- *(real-provider only; the Java live fixture on Windows. Until this change the bridge's description is listed only as `native:Description`)*

#### Scenario: The Java agent sources Swing's description
- **GIVEN** the same button served through the Java agent
- **WHEN** its `@Description` and its description accessor are read
- **THEN** both SHALL be `A button with a developer name`
- *(real-provider only; the Java agent on Windows)*

#### Scenario: A Swing tool tip is the description Swing derives
- **GIVEN** a Swing component with the tool tip text `Save the file` and no accessible description set
- **WHEN** it is described by the Java agent
- **THEN** its description SHALL be `Save the file`, because Swing reports it as the accessible description
- *(verified on the agent's description in isolation)*
