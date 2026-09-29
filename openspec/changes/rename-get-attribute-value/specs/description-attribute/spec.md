# Spec Delta

## MODIFIED Requirements

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
