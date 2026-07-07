# textcontent-pattern Specification

## Purpose
TBD - created by syncing change add-textcontent-pattern. Update Purpose after archive.
## Requirements
### Requirement: TextContent exposes an element's text as a read-only Text attribute

The UI model SHALL surface the current textual content of a text-bearing element as a read-only `control:Text` string attribute. The content SHALL be sourced only from the element's accessibility text interface — the AT-SPI `Text` interface (`GetText(0,-1)`), or on Windows the UIA `TextPattern` document text, falling back to the `ValuePattern` value when there is no TextPattern. It SHALL NOT fall back to the element's accessible name/label. An element that exposes no text interface SHALL NOT expose `control:Text`.

#### Scenario: A text-bearing element exposes its content

- **WHEN** an element that implements a text interface with content "Hello" is queried
- **THEN** its `control:Text` attribute SHALL equal "Hello"
- *(Verifiable only against a real provider — AT-SPI.)*

#### Scenario: An empty text field still exposes Text

- **WHEN** an element implements a text interface but currently holds no text
- **THEN** its `control:Text` attribute SHALL be present and empty, not absent
- *(Verifiable only against a real provider.)*

#### Scenario: An element without a text interface has no Text

- **WHEN** an element exposes no accessibility text interface
- **THEN** it SHALL NOT expose a `control:Text` attribute, even if it has an accessible name
- *(Verifiable only against a real provider.)*

#### Scenario: Text is not sourced from the accessible name

- **WHEN** a plain label or button carries text only in its accessible name and implements no text interface
- **THEN** `control:Text` SHALL be absent and the label text SHALL remain available via `control:Name`
- *(Verifiable only against a real provider.)*

### Requirement: TextContent is read-only and carries no writability information

The `TextContent` capability SHALL be a read-only contract. It SHALL NOT define an action, SHALL NOT be retrievable as a runtime pattern instance, and SHALL NOT expose an `IsReadOnly` attribute or any read-only/editable flag. Whether the text can be edited is out of scope for `TextContent`.

#### Scenario: TextContent provides only text content

- **WHEN** a node's `TextContent` capability is inspected
- **THEN** it SHALL provide the text content only, with no `IsReadOnly` or editability attribute attached to the `TextContent` contract

### Requirement: TextContent is available wherever the Text attribute is present

`TextContent` SHALL be treated as supported by any node that exposes a `control:Text` attribute, without the native provider having to advertise the pattern name in its supported-patterns list. The Robot Framework `Text` context class SHALL read its `text` through this capability.

#### Scenario: TextContent is synthesized from the Text attribute

- **WHEN** a node exposes a `control:Text` attribute
- **THEN** `supports_pattern(TextContent)` SHALL be true and `get_pattern(TextContent).text` SHALL return the attribute's value
- **AND** this SHALL hold even though the native supported-patterns list does not contain `TextContent`

#### Scenario: The read-only text widget reads its content

- **WHEN** a `Text` widget wrapping an element that exposes `control:Text` is asked for its text
- **THEN** it SHALL return the element's current text content
