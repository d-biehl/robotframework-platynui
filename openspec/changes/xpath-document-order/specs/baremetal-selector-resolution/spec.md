# Spec Delta

## ADDED Requirements

### Requirement: Several matches resolve to the first in document order

When a selector matches several elements, a keyword that acts on one element, `Query` with `only_first=${True}`, and the waits SHALL use the first match in document order. `Query` without `only_first` SHALL list the matches in document order, each once. A positional predicate inside a step of the selector SHALL count per parent, while a positional predicate on a parenthesized selector SHALL count over all matches. The XPath rules behind this are those of the `xpath-evaluation` capability. The scenarios are verifiable on the mock; they hold for every provider, because all providers share the engine.

#### Scenario: A keyword acts on the first match in document order

- **GIVEN** the mock, and the root set to `//control:Window[@Name="Operations Console"]`, whose tree holds the tree items Dashboard (with Overview and Metrics) and Reports (with Täglich, Monatlich and Jährlich)
- **WHEN** `Get Attribute Value    .//item:TreeItem[@Name!="Dashboard"]    Name` runs
- **THEN** it SHALL return Overview
- **NOTE:** Before this change it returns Reports, the first match of the shallower level.

#### Scenario: Query only_first returns the first match in document order

- **GIVEN** the same root
- **WHEN** `Query    .//item:TreeItem[@Name!="Dashboard"]    only_first=${True}` runs
- **THEN** it SHALL return the element named Overview
- **NOTE:** Before this change Reports.

#### Scenario: Query lists matches in document order

- **GIVEN** the same root
- **WHEN** `Query    .//item:TreeItem` runs
- **THEN** the names of the returned elements SHALL be Dashboard, Overview, Metrics, Reports, Täglich, Monatlich, Jährlich, in that order
- **NOTE:** Before this change Dashboard, Reports, Overview, Metrics, Täglich, Monatlich, Jährlich.

#### Scenario: A parenthesized selector counts over all matches

- **GIVEN** the same root
- **WHEN** `Get Attribute Value    (.//item:TreeItem)[2]    Name    ==    Overview` runs
- **THEN** it SHALL pass
- **NOTE:** Before this change the selector resolves Reports.

#### Scenario: A positional predicate in a step counts per parent

- **GIVEN** the same root
- **WHEN** `Query    .//item:TreeItem[2]` runs
- **THEN** it SHALL return the elements named Metrics, Reports and Monatlich, in that order
- **NOTE:** Before this change only Reports.

#### Scenario: A position that no parent reaches matches nothing

- **GIVEN** the same root and `query_settings={'timeout': 0.2}`
- **WHEN** `Get Attribute Value    .//item:TreeItem[4]    Name` runs
- **THEN** it SHALL fail with an error that says no element matched `.//item:TreeItem[4]` within timeout of 0.2 seconds
- **NOTE:** No parent holds four tree items. Before this change the selector resolves Metrics, the fourth match overall.
