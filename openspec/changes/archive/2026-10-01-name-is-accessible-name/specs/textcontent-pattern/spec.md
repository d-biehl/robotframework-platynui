# Spec Delta

## MODIFIED Requirements

### Requirement: TextContent exposes an element's text as a read-only Text attribute

The UI model SHALL surface the current textual content of a text-bearing element as a read-only `control:Text` string attribute. The content SHALL be sourced only from the element's accessibility text interface:

- the AT-SPI `Text` interface (`GetText(0,-1)`);
- on Windows, the UIA `TextPattern` document text, falling back to the `ValuePattern` value when there is no TextPattern;
- through the Java agent, the element's Swing text: a text component's document text, and otherwise the `AccessibleText` Swing provides, plain or extended. For a table cell, a column header or a list entry this is the `AccessibleText` Swing provides for that item. Swing's labels and buttons, and the renderers built on them, provide one only for HTML text, so a plain-text label, button, cell, header or list entry has no text interface.

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

#### Scenario: A plain Swing label has no Text and does not borrow its name

- **GIVEN** the Swing test application's stage-1 status label, which displays `clicks-0` without HTML and whose accessible name is `stage1-status-clicks-0`
- **WHEN** the label's attributes are read through the Java agent
- **THEN** it SHALL NOT expose `control:Text`, and its `control:Name` SHALL be `stage1-status-clicks-0`
- *(Verifiable only against a real provider — the Java agent on Windows. Swing's label provides a text interface only for HTML text.)*

#### Scenario: A plain Swing button has no Text

- **GIVEN** the Swing test application's stage-1 button, which displays `Click me` without HTML
- **WHEN** the button's attributes are read through the Java agent
- **THEN** it SHALL NOT expose `control:Text`
- *(Verifiable only against a real provider.)*

#### Scenario: Plain Swing table cells and column headers have no Text

- **GIVEN** the Swing test application's `names-table`, whose `amount` cell displays `1,234.50`, whose `active` cell displays a check box, and whose first column header displays `amount`, all without HTML
- **WHEN** their attributes are read through the Java agent
- **THEN** none of them SHALL expose `control:Text`, and the `amount` cell's `control:Name` SHALL be `1,234.50`
- *(Verifiable only against a real provider.)*

#### Scenario: An HTML Swing label exposes the text Swing reports

- **GIVEN** a Swing label whose text is `<html>Hi <b>there</b></html>`
- **WHEN** it is described by the Java agent
- **THEN** its text SHALL be what its `AccessibleText` reports: `Hi there`, preceded by the line break the HTML document starts with
- *(Verified on the agent's description in isolation.)*

#### Scenario: An HTML item exposes its text through Swing's item wrapper

- **GIVEN** a Swing list whose renderer draws its first entry from `<html>Alpha</html>` and its second entry `Beta` without HTML
- **WHEN** both entries are described by the Java agent
- **THEN** the first entry's text SHALL be `Alpha`, preceded by that line break, and the second entry SHALL carry no text
- *(Verified on the agent's description in isolation; the fixture has no HTML item.)*
