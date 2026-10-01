# core-locator Specification

## Purpose

What a PlatynUI.core `Locator` selects and how it is rendered as XPath: its scope, its `index` and its `position`, relative to the one parent element it is resolved against. Users set these fields without writing XPath, so their meaning is stated here in terms of elements rather than XPath steps.

## Requirements

### Requirement: A locator's index and position count over the whole scope

A locator is resolved against one parent element. Its `index` SHALL select the n-th element, in document order, of the elements that its scope, node name and predicates select from that parent. Its `position` SHALL select by the element's position among all elements of the scope with that node name, in document order. On the `descendants` scope this SHALL count over all descendants of the parent, not per intermediate parent. Custom predicates on the `descendants` scope SHALL see the same positions. On the reverse scopes (`parent`, `ancestor`, `ancestor-or-self`, `preceding`, `preceding-sibling`), `index` SHALL keep counting from the nearest element outward. A locator with a `path` or an explicit `axis` SHALL be rendered as given, without a change of meaning.

#### Scenario: index on the descendants scope renders a single descendant step

- **GIVEN** `Locator(role='Button', index=2)` and `Locator(role='Button', position=3, index=1)`
- **WHEN** they are rendered with `to_xpath()`
- **THEN** the XPaths SHALL be `descendant::Button[2]` and `descendant::Button[position()=3][1]`

#### Scenario: Custom predicates on the descendants scope render a single descendant step

- **GIVEN** `Locator(role='X', custom_attributes=["@Foo='bar'", 'position()=2'])`
- **WHEN** it is rendered
- **THEN** the XPath SHALL be `descendant::X[@Foo='bar' and position()=2]`

#### Scenario: A locator without index, position or custom predicates renders a plain descendant step

- **GIVEN** `Locator(role='Button', name='OK')`
- **WHEN** it is rendered
- **THEN** the XPath SHALL be `.//Button[@Name="OK"]`

#### Scenario: The other scopes render a single step

- **GIVEN** `Locator(role='Button', index=2)` rendered with `parent_is_root_like=True`, `Locator(role='Window', scope='root', index=1)`, `Locator(role='Pane', scope='ancestor', index=1)` and `Locator(role='Button', scope='preceding-sibling', index=1)`
- **WHEN** they are rendered
- **THEN** the XPaths SHALL be `Button[2]`, `/Window[1]`, `ancestor::Pane[1]` and `preceding-sibling::Button[1]`
- **NOTE:** Each is one step from one parent element, so its positions count over the whole scope; `ancestor::Pane[1]` is the nearest ancestor.

#### Scenario: A path and an explicit axis are rendered as given

- **GIVEN** `Locator(path='.//Button[2]', index=5)` and `Locator(role='Button', axis='.//', index=2)`
- **WHEN** they are rendered
- **THEN** the XPaths SHALL be `.//Button[2]` and `.//Button[2]`
- **NOTE:** They are XPath the user wrote, and they follow the XPath 2.0 rules of the `xpath-evaluation` capability.

#### Scenario: index selects the n-th descendant overall

- **GIVEN** the mock provider, and the `Operations Console` window as the parent, whose tree holds the tree items Dashboard (with Overview and Metrics) and Reports (with Täglich, Monatlich and Jährlich)
- **WHEN** `find_one` resolves `Locator(prefix='item', role='TreeItem', index=2)` against that window
- **THEN** it SHALL return the element named Overview

#### Scenario: An index past the last descendant finds nothing

- **GIVEN** the same parent
- **WHEN** `find_one` resolves `Locator(prefix='item', role='TreeItem', index=8)`
- **THEN** it SHALL return None

#### Scenario: All matches come back in document order

- **GIVEN** the same parent
- **WHEN** `find_all` resolves `Locator(prefix='item', role='TreeItem')`
- **THEN** the names SHALL be Dashboard, Overview, Metrics, Reports, Täglich, Monatlich, Jährlich, in that order
