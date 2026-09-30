# Spec Delta

## MODIFIED Requirements

### Requirement: TextContent exposes an element's text as a read-only Text attribute

The UI model SHALL surface the current textual content of a text-bearing element as a read-only `control:Text` string attribute. The content SHALL be sourced only from the element's accessibility text interface:

- the AT-SPI `Text` interface (`GetText(0,-1)`);
- on Windows, the UIA `TextPattern` document text, falling back to the `ValuePattern` value when there is no TextPattern;
- through the Java agent, the element's Swing text: a text component's document text, and otherwise its `AccessibleText`. For a table cell, a column header or a list entry this is the `AccessibleText` Swing provides for that item, which is the text its renderer displays.

It SHALL NOT fall back to the element's accessible name/label. An element that exposes no text interface SHALL NOT expose `control:Text`.

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

#### Scenario: A Swing label exposes its text, not its accessible name

- **GIVEN** the Swing test application's stage-1 status label, which displays `clicks-0` and whose accessible name is `stage1-status-clicks-0`
- **WHEN** the label's `control:Text` is read through the Java agent
- **THEN** it SHALL be `clicks-0`
- *(Verifiable only against a real provider — the Java agent on Windows. Swing's label implements a plain text interface without the extended one.)*

#### Scenario: A Swing button exposes its label as Text

- **GIVEN** the Swing test application's stage-1 button, which displays `Click me` and whose accessible name is `stage1-button`
- **WHEN** the button's `control:Text` is read through the Java agent
- **THEN** it SHALL be `Click me`
- *(Verifiable only against a real provider.)*

#### Scenario: A Swing table cell exposes the text it displays

- **GIVEN** the Swing test application's `names-table`, whose `amount` cell holds the model value `1234.5` and displays `1,234.50`
- **WHEN** the cell's `control:Text` is read through the Java agent
- **THEN** it SHALL be `1,234.50`
- *(Verifiable only against a real provider — the Java agent on Windows.)*

#### Scenario: A Swing table cell that displays no text exposes empty Text

- **GIVEN** the Swing test application's `names-table`, whose `active` cell is displayed as a check box without text
- **WHEN** the cell's attributes are read through the Java agent
- **THEN** `control:Text` SHALL be present and empty, because the check box renderer implements a text interface
- *(Verifiable only against a real provider.)*

#### Scenario: A Swing column header exposes the text it displays

- **GIVEN** the Swing test application's `main-table`, whose second column header displays `col-1`
- **WHEN** the header's `control:Text` is read through the Java agent
- **THEN** it SHALL be `col-1`
- *(Verifiable only against a real provider.)*

#### Scenario: A Swing list entry exposes the text it displays

- **GIVEN** a Swing list whose renderer displays `Alpha` for its first entry
- **WHEN** the entry is described by the Java agent
- **THEN** its text SHALL be `Alpha`
- *(Verified on the agent's description in isolation; the fixture has no list outside a combo box popup.)*

#### Scenario: A Swing cell's text reaches the TextContent capability

- **GIVEN** a table cell served by the Java agent that exposes `control:Text`
- **WHEN** `supports_pattern(TextContent)` and `get_pattern(TextContent).text` are asked of its node
- **THEN** the pattern SHALL be supported and its text SHALL equal the cell's `control:Text`
- *(Verifiable only against a real provider; the synthesis itself is covered by the requirement below.)*
