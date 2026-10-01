# Spec Delta

## MODIFIED Requirements

### Requirement: Strict per-platform source mapping
Each provider SHALL source `Description` exclusively from its platform's accessible-description property: AT-SPI2 `Accessible.Description`; Windows UIA `FullDescription` (`UIA_FullDescriptionPropertyId`). Providers MUST NOT fall back to help/tooltip-like properties (`Accessible.HelpText`, UIA `HelpText`, `LegacyIAccessible.Description`); those remain available under the `native:` namespace only. The macOS AX provider is a stub and emits no `Description`.

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
- *(verified on the provider's decision in isolation; today AT-SPI trims it)*

#### Scenario: A whitespace-only AT-SPI description is listed as reported
- **GIVEN** an AT-SPI element whose `Accessible.Description` consists of three spaces
- **WHEN** its `Description` is decided
- **THEN** it SHALL be the three spaces, not absent
- *(verified on the provider's decision in isolation; today AT-SPI drops it)*

#### Scenario: The native AT-SPI description is the description as reported
- **GIVEN** an AT-SPI element whose `Accessible.Description` is `Closes the dialog ` with a trailing space
- **WHEN** its `native:Accessible.Description` is read
- **THEN** it SHALL be `Closes the dialog `, equal to its `control:Description`
- *(verified on the provider's mapping in isolation; today the native value is trimmed as well)*
