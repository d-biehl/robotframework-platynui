# description-attribute Specification

## Purpose
The accessible description as a provider-independent `control:Description` attribute.
Accessibility APIs expose a description beside the name and id (AT-SPI
`Accessible.Description`, UIA `FullDescription`), but PlatynUI surfaced it only as a
provider-specific `native:` attribute, so no suite could locate or assert on it portably.
This capability fixes the source per platform strictly — no falling back to help text or
legacy fields — emits the attribute only when the platform reports a non-empty value, and
adds a first-class accessor alongside name and id.

## Requirements
### Requirement: Description is a common control-namespace attribute
The system SHALL define `Description` as a common attribute in the `control:` namespace (canonical constant `attribute_names::common::DESCRIPTION`), available on every `control:`/`item:` node whose underlying platform element exposes a non-empty accessible description. The attribute value SHALL be the platform's accessible-description string, unmodified.

#### Scenario: Description is queryable via XPath
- **GIVEN** a UI element whose platform accessible description is "Closes the dialog without saving"
- **WHEN** the locator `//control:Button[@Description='Closes the dialog without saving']` is evaluated
- **THEN** the element is found

#### Scenario: Description is readable via attribute lookup
- **GIVEN** an element with a non-empty accessible description
- **WHEN** `attribute("Description")` is read in the control namespace (e.g. RF `Get Attribute Value`)
- **THEN** the platform's description string is returned

### Requirement: Description is emitted only when non-empty
Providers SHALL NOT emit the `Description` attribute when the platform value is absent or an empty string (the same presence rule as `control:Id`). Absence of a description is normal, expected behavior — not an error.

#### Scenario: Empty description means absent attribute
- **GIVEN** an element whose platform accessible description is empty or unset
- **WHEN** the element's attributes are enumerated
- **THEN** no `control:Description` attribute is present
- **AND** the predicate `[@Description]` does not match the element

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
- *(real-provider only; the Java live fixture on Windows)*

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

### Requirement: First-class description accessor
The `UiNode` trait SHALL provide a `description()` accessor returning an optional string (defaulting to none), and the value SHALL be exposed through the native Python bindings and as a read-only `description` property on the Python `Adapter` and `Context` APIs, consistent with the existing `name`/`id` accessors.

#### Scenario: Python context exposes description
- **GIVEN** a resolved element whose provider emits `control:Description` with value "Primary action"
- **WHEN** the `description` property is read on the Python context/adapter
- **THEN** it returns "Primary action"

#### Scenario: Python description is None when absent
- **GIVEN** a resolved element without a `control:Description` attribute
- **WHEN** the `description` property is read
- **THEN** it returns `None` (no exception)

### Requirement: Mock provider supports spec-driven descriptions
The mock provider SHALL emit `control:Description` (and return it from `description()`) when the mock tree spec defines a description for a node, and SHALL omit it otherwise, so the fast mock lane can cover attribute plumbing. The mock is deliberately partial and is not authoritative for platform mapping semantics.

#### Scenario: Mock node with description
- **GIVEN** a mock tree spec that assigns description "Demo description" to a button node
- **WHEN** `//control:Button[@Description='Demo description']` is evaluated against the mock provider
- **THEN** the node is found and its `description()` accessor returns "Demo description"

#### Scenario: Mock node without description
- **GIVEN** a mock tree spec node with no description
- **WHEN** the node's attributes are enumerated
- **THEN** no `control:Description` attribute is present

### Requirement: End-to-end acceptance coverage on AT-SPI
The egui test application SHALL set an AccessKit description on at least one stable, locatable widget, and the Linux acceptance suite SHALL verify that this description is readable as `control:Description` through the real AT-SPI provider. Acceptance tests MUST stay provider-independent in their locators (no `native:` attributes).

#### Scenario: Description round-trip through the real provider
- **GIVEN** the egui test app running in the compositor acceptance session with a widget whose AccessKit description is set
- **WHEN** the acceptance test locates the widget and reads its `Description` attribute
- **THEN** the configured description string is returned
- *(real-provider only: this is the authoritative check; the mock lane covers plumbing only)*
