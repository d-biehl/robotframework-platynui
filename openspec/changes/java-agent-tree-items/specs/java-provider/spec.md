## ADDED Requirements

### Requirement: Hierarchical content served by the agent is a real hierarchy
A tree surfaced through the agent backend SHALL report its content as nested nodes, each node carrying its own children, rather than as a single level of accessibility wrappers. Node structure, names, expansion state and selection SHALL come from the toolkit's own model, because the accessibility view of a Swing tree is measurably wrong on all four counts: it truncates below the first level, announces children it cannot name or place, derives roles from the cell renderer, and reports a selection that only ever considers the root.

A node that is **not realized on screen** — one inside a collapsed branch — SHALL NOT appear in the tree. A node that can be expanded SHALL say so, so a consumer can expand it and then find what it contains. This keeps the invariant that every node in the tree is one a user could see and point at.

Each node SHALL keep its identity across expansions and collapses elsewhere in the tree, so a reference taken before a structural change still resolves to the same node afterwards.

#### Scenario: A tree reports its nested structure
- **WHEN** a Swing tree with an expanded branch and a collapsed branch is enumerated through the agent backend
- **THEN** the tree's nodes are nested according to the model — a node's children are reachable beneath it — and not presented as one flat or single-level list

#### Scenario: A collapsed branch hides its contents but says it has some
- **WHEN** a collapsed node is inspected
- **THEN** it reports that it can be expanded and is currently not expanded, and it exposes no children
- **AND** no node without an on-screen rectangle appears anywhere in the tree

#### Scenario: Expanding a branch reveals its nodes
- **WHEN** a collapsed node is expanded and the tree is enumerated again
- **THEN** its children are present, each with a name, an on-screen rectangle and its own expansion state

#### Scenario: A node keeps its identity across structural changes
- **WHEN** a branch elsewhere in the tree is expanded or collapsed, shifting the on-screen position of a node
- **THEN** that node's identity is unchanged, so a reference taken beforehand still resolves to it

#### Scenario: Selection reflects the model
- **WHEN** a node in the tree is selected
- **THEN** the node reports itself as selected and the tree publishes an identifier for it that resolves to that node — not merely whichever selection the accessibility view happens to acknowledge

#### Scenario: Hit-testing reaches the node
- **WHEN** a point over a tree row is hit-tested through the agent backend
- **THEN** the returned chain reaches that node through its ancestors, and the node is the same one the enumeration produces

#### Scenario: Tree nodes are recognizable as tree items
- **WHEN** any node of a tree is inspected
- **THEN** its role identifies it as a tree item rather than as whatever component the tree happens to render its rows with
