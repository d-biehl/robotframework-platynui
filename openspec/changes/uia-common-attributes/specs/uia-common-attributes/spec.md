# UIA Common Attributes

## ADDED Requirements

### Requirement: UIA element nodes expose Technology
Every `control:`/`item:` node produced by the windows-uia provider SHALL expose a `control:Technology` attribute with the value `UIAutomation`, both in the `attributes()` enumeration and via the `attribute()` single lookup.

#### Scenario: Technology on a window node
- **WHEN** a top-level window node from the windows-uia provider is enumerated
- **THEN** its attributes contain `control:Technology` = `UIAutomation`

#### Scenario: Technology via XPath filter
- **WHEN** a query selects `/Window[@Technology="UIAutomation"]` on a Windows desktop with at least one non-claimed top-level window
- **THEN** the query matches the UIA representation of that window

### Requirement: UIA element nodes expose SupportedPatterns
Every `control:`/`item:` node produced by the windows-uia provider SHALL expose a `control:SupportedPatterns` attribute whose value is derived from the node's `supported_patterns()` result, so the attribute and the pattern advertisement cannot diverge.

#### Scenario: SupportedPatterns matches pattern advertisement
- **WHEN** the `control:SupportedPatterns` attribute of a UIA node is read
- **THEN** its entries equal the node's `supported_patterns()` list converted with `supported_patterns_value`

#### Scenario: Window node advertises window patterns
- **WHEN** the `control:SupportedPatterns` attribute of a UIA top-level window is read
- **THEN** it contains the window management patterns (Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable, Responsive)

### Requirement: UIA application nodes carry the common attributes
The synthetic `ApplicationNode` of the windows-uia provider SHALL expose `control:Technology` = `UIAutomation` and `control:SupportedPatterns` (reflecting its `supported_patterns()` result) alongside its existing common attributes.

#### Scenario: Application node attributes
- **WHEN** the attributes of a UIA application node are enumerated
- **THEN** they include `control:Technology` = `UIAutomation` and a `control:SupportedPatterns` attribute

### Requirement: Focusable is gated on keyboard focusability
The windows-uia provider SHALL advertise the `Focusable` pattern and return its action according to the UIA `IsKeyboardFocusable` property, taking an explicit value at face value. Because that property's documented default value is `FALSE`, an unimplemented property is indistinguishable from a denial through the plain accessor; the provider SHALL therefore read it in a way that separates the two (`ignoreDefaultValue`) and SHALL NOT treat a missing value as a denial. When no value is supplied, a top-level window SHALL keep advertising `Focusable`, while an element deeper in the tree SHALL not.

#### Scenario: Non-focusable element
- **WHEN** an element reports `IsKeyboardFocusable` = false (e.g. a static text label)
- **THEN** `Focusable` is absent from its `SupportedPatterns` and `pattern_by_name(Focusable)` returns no action

#### Scenario: Focusable element
- **WHEN** an element reports `IsKeyboardFocusable` = true
- **THEN** `Focusable` appears in its `SupportedPatterns` and `pattern_by_name(Focusable)` returns the focus action

#### Scenario: Provider supplies no value for a top-level window
- **WHEN** a top-level window's provider does not implement `IsKeyboardFocusable` at all (the read yields the NotSupported sentinel rather than a boolean)
- **THEN** `Focusable` remains advertised and `pattern_by_name(Focusable)` still returns the focus action, rather than being withdrawn on the strength of the property's `FALSE` default

#### Scenario: Provider supplies no value for an inner element
- **WHEN** an element that is not a top-level window yields the NotSupported sentinel for `IsKeyboardFocusable`
- **THEN** `Focusable` is absent from its `SupportedPatterns`

### Requirement: TextEditable capability marker
The windows-uia provider SHALL advertise the `TextEditable` pattern as a capability marker for elements that support text content (`TextPattern` or `ValuePattern` available) and are not read-only. The provider SHALL NOT return a programmatic set-text action for this pattern; text entry remains keyboard-driven.

#### Scenario: Editable text field
- **WHEN** an element supports `ValuePattern` with `IsReadOnly` = false
- **THEN** `TextEditable` appears in its `SupportedPatterns` and `pattern_by_name(TextEditable)` returns no action instance

#### Scenario: Read-only text element
- **WHEN** an element supports only `TextPattern`, or `ValuePattern` reports `IsReadOnly` = true
- **THEN** `TextEditable` is absent from its `SupportedPatterns`

### Requirement: IsReadOnly attribute on text-bearing elements
The windows-uia provider SHALL expose a `control:IsReadOnly` attribute on every element that supports text content. The value SHALL come from `ValuePattern.IsReadOnly` when `ValuePattern` is available; elements with only `TextPattern` SHALL report `IsReadOnly` = true.

#### Scenario: Editable field reports IsReadOnly false
- **WHEN** an edit control with a writable `ValuePattern` is inspected
- **THEN** `control:IsReadOnly` is present with value false

#### Scenario: Non-text element has no IsReadOnly
- **WHEN** an element supports neither `TextPattern` nor `ValuePattern`
- **THEN** no `control:IsReadOnly` attribute is exposed

### Requirement: Shared contract expectation for common attributes
The core contract testkit SHALL provide a reusable expectation asserting the always-present common attributes (`Role`, `Name`, `RuntimeId`, `Technology`, `SupportedPatterns`) on `control:`/`item:` nodes, usable by provider test suites.

#### Scenario: Conforming node passes
- **WHEN** a node exposing all always-present common attributes is verified against the common-attribute expectation
- **THEN** no contract issues are reported

#### Scenario: Missing Technology is detected
- **WHEN** a node lacking `control:Technology` is verified against the common-attribute expectation
- **THEN** a missing-attribute issue for `Technology` is reported
