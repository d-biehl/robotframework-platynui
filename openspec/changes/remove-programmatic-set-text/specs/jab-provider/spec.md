# JAB Provider — Delta

## MODIFIED Requirements

### Requirement: Core interaction patterns
The provider SHALL support: Focusable (`requestFocus`), ActivationTarget (bounds center), TextContent (chunked `getAccessibleTextRange`), TextEditable (capability marker from the text interface and `editable` state — no write action, per the text-input-policy capability), Toggleable (`checked` state), StatefulValue (numeric value/min/max), Selectable/SelectionProvider (`AccessibleSelection`), and Expandable — each advertised only when the underlying JAB interfaces/states genuinely back it.

#### Scenario: TextEditable is a marker without an action
- **WHEN** the fixture's editable stage-1 text field is inspected
- **THEN** it advertises `TextEditable` with `IsReadOnly` = false, and `pattern_by_name(TextEditable)` returns no action instance

#### Scenario: Toggle state reflects reality
- **WHEN** the fixture checkbox is activated once (e.g. a pointer click)
- **THEN** its `ToggleState` changes from Off to On, read back on the same runtime (the provider reads JAB state live per access)

#### Scenario: Honest pattern lists
- **WHEN** a node without an accessible-text interface is inspected
- **THEN** it advertises no TextContent pattern and exposes no `control:Text`
