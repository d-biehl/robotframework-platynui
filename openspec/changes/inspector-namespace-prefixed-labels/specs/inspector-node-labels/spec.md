# inspector-node-labels Delta

## ADDED Requirements

### Requirement: Node labels show the XPath-qualified role

The Inspector SHALL label a node with its role written the way an XPath name test must write it to match the node: prefixed with the node's namespace (`ns:Role`) for every namespace except the default `control` namespace, and unprefixed (`Role`) for `control`. A non-empty name SHALL follow the role, separated by a space and enclosed in double quotes, with control characters escaped. A node with an empty name SHALL be labelled with the qualified role alone.

#### Scenario: Item node shows the item prefix

- **GIVEN** a node in the `item` namespace with role `ListItem` and name `Item 1`
- **WHEN** the Inspector builds its label
- **THEN** the label SHALL be `item:ListItem "Item 1"`

#### Scenario: Control node shows no prefix

- **GIVEN** a node in the `control` namespace with role `Button` and name `OK`
- **WHEN** the Inspector builds its label
- **THEN** the label SHALL be `Button "OK"`

#### Scenario: App node shows the app prefix

- **GIVEN** a node in the `app` namespace with role `Application` and name `MyApp`
- **WHEN** the Inspector builds its label
- **THEN** the label SHALL be `app:Application "MyApp"`

#### Scenario: Native node shows the native prefix

- **GIVEN** a node in the `native` namespace with role `Widget` and name `Custom`
- **WHEN** the Inspector builds its label
- **THEN** the label SHALL be `native:Widget "Custom"`

#### Scenario: Empty name shows only the qualified role

- **GIVEN** a node in the `item` namespace with role `TableRow` and an empty name
- **WHEN** the Inspector builds its label
- **THEN** the label SHALL be `item:TableRow`, without quotes

#### Scenario: Control characters in the name stay escaped

- **GIVEN** a node in the `item` namespace with role `ListItem` and the name `Line 1\nLine 2` (containing a line feed)
- **WHEN** the Inspector builds its label
- **THEN** the label SHALL be `item:ListItem "Line 1 Line 2"`

#### Scenario: Qualified role works as an XPath name test

- **GIVEN** a node in the `item` namespace shown in the Inspector tree as `item:ListItem "Item 1"`
- **WHEN** the user evaluates `//item:ListItem[@Name="Item 1"]` in the Inspector's XPath search
- **THEN** the result list SHALL contain that node
- **NOTE** Verified manually against a real provider with an application that exposes list items. The XPath engine's default element namespace is outside this change; the unit tests cover the label format only.

### Requirement: XPath result nodes carry the same label as the tree

A node returned by an XPath search SHALL be labelled in the result list exactly as the same node is labelled in the UI tree.

#### Scenario: Result label equals tree label

- **GIVEN** a node in the `item` namespace with role `ListItem` and name `Item 1`
- **WHEN** the Inspector builds the tree label for the node and the result-list label for the same node returned as an XPath node result
- **THEN** both labels SHALL be `item:ListItem "Item 1"`
