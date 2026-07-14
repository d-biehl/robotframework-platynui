# menu-item-namespace Specification

## ADDED Requirements

### Requirement: Menu-entry roles are classified in the control namespace

Menu-entry roles SHALL be exposed in the default `control` XPath namespace, not the `item` namespace. This applies to `MenuItem` and its variants that a provider surfaces as menu entries: on AT-SPI these are the roles `MenuItem`, `CheckMenuItem`, `RadioMenuItem` (all emitted under the role name `MenuItem`), and `TearoffMenuItem` (emitted as `TearoffMenuItem`). A menu entry is an invokable control that may own a submenu; it is not a data item belonging to a collection container, so it MUST NOT live in the `item` namespace.

The menu container roles — `Menu`, `MenuBar`, `PopupMenu` — SHALL remain in the `control` namespace (unchanged).

#### Scenario: A menu item resolves in the control namespace

- **WHEN** an application menu is opened and a menu entry is queried
- **THEN** the entry SHALL be matched by `//MenuItem` (the default `control` namespace)
- **AND** `//item:MenuItem` SHALL NOT match it
- *(Verifiable against a real provider — AT-SPI.)*

#### Scenario: Check and radio menu entries behave like plain menu items

- **WHEN** a checkable or radio menu entry is queried
- **THEN** it SHALL be exposed under the role name `MenuItem` in the `control` namespace, the same as a plain menu entry

#### Scenario: Menu containers stay in the control namespace

- **WHEN** a `Menu`, `MenuBar`, or `PopupMenu` is queried
- **THEN** it SHALL be matched in the default `control` namespace, unchanged by this capability

### Requirement: Menu-item namespace is consistent across providers

The namespace of a menu entry SHALL NOT depend on which platform accessibility technology surfaced it. A locator written against a menu entry SHALL be portable between the AT-SPI provider and the Windows UIA provider without changing its namespace prefix.

#### Scenario: The same locator matches menu items on AT-SPI and Windows UIA

- **WHEN** the same menu-entry locator `//MenuItem` is evaluated against the AT-SPI provider and against the Windows UIA provider
- **THEN** it SHALL resolve the menu entry on both, because both classify menu entries in the `control` namespace
- *(Windows UIA already derives `control` from `IsControlElement`; verifiable only against real providers.)*

### Requirement: Collection data-item roles remain in the item namespace

Reclassifying menu entries SHALL NOT change any other role. Roles that genuinely represent data items of a collection container — including `ListItem`, `TreeItem`, `TableCell`, `TableRow`, `TabItem`, and the column/row header roles — SHALL remain in the `item` namespace.

#### Scenario: List and tree items are unaffected

- **WHEN** a list row or tree node is queried
- **THEN** it SHALL still be matched by `//item:ListItem` / `//item:TreeItem` respectively
- *(Verifiable against a real provider — AT-SPI.)*
