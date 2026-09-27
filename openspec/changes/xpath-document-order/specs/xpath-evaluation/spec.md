# Spec Delta

## Purpose

What the XPath engine returns for a path expression: its nodes in document order without duplicates, positional predicates counted per context node, and predicates on a parenthesized expression counted over the whole sequence. It also covers what finding the first result may read of the tree, which the keywords that act on one element depend on.

## ADDED Requirements

### Requirement: A path returns its nodes in document order without duplicates

The engine SHALL return the nodes that a path expression `E1/E2` selects in document order, each node once. This SHALL hold for every step shape: child, self, attribute, descendant, descendant-or-self, following, following-sibling, parent, ancestor, ancestor-or-self, preceding, preceding-sibling, a filter-expression step, and a path whose first operand is an arbitrary node sequence. It SHALL hold whether or not the model's nodes carry document-order keys. Nodes of different trees in one result SHALL keep a stable order without an error. A final step that yields atomic values SHALL keep their order.

#### Scenario: Matches at different depths come out in document order

- **GIVEN** the tree `r:[A:[X1],X2]`
- **WHEN** `//X` is evaluated
- **THEN** the result SHALL be `[X1, X2]`
- **NOTE:** Before this change `[X2, X1]`.

#### Scenario: A descendant search over nested parents is in document order

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** `//B` is evaluated
- **THEN** the result SHALL be `[B1, B2, B3, B4, B5]`
- **NOTE:** Before this change `[B3, B1, B2, B4, B5]`.

#### Scenario: A child step over nested context nodes is in document order

- **GIVEN** the tree `r:[X1:[X2:[Y1],Y2]]`
- **WHEN** `//X/Y` is evaluated
- **THEN** the result SHALL be `[Y1, Y2]`
- **NOTE:** Before this change `[Y2, Y1]`.

#### Scenario: Sibling and child steps from several context nodes are in document order

- **GIVEN** the tree `r:[a:[b1,b2],c]`
- **WHEN** `//*/following-sibling::*` and `//*/child::*` are evaluated
- **THEN** the results SHALL be `[b2, c]` and `[a, b1, b2, c]`
- **NOTE:** Before this change `[c, b2]` and `[a, c, b1, b2]`.

#### Scenario: following from nested context nodes keeps every result

- **GIVEN** the tree `r:[a1:[a2,c1]]`
- **WHEN** `//a/following::c` is evaluated
- **THEN** the result SHALL be `[c1]`
- **NOTE:** Before this change empty.

#### Scenario: A path from an arbitrary sequence is ordered and duplicate-free

- **GIVEN** the tree `r:[a,b,c]`
- **WHEN** `(//b, //c, //a)/.` and `(//c, //a, //c)/self::*` are evaluated
- **THEN** the results SHALL be `[a, b, c]` and `[a, c]`
- **NOTE:** Before this change `[b, a, c]` and `[c, a, c]`.

#### Scenario: A parent step from nodes given out of order is in document order

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** `(//B[@id='B1'], //B[@id='B4'], //B[@id='B3'])/..` is evaluated
- **THEN** the result SHALL be `[W, P1, P2]`
- **NOTE:** Before this change `[P1, W, P2]`.

#### Scenario: Keyless models give the same results

- **GIVEN** each tree above in a model whose nodes carry no document-order key
- **WHEN** the same expressions are evaluated
- **THEN** the results SHALL equal those of the keyed model

#### Scenario: A node missing from its parent's list does not break the order

- **GIVEN** a keyless model in which one node is missing from its parent's list of children
- **WHEN** a path that has to sort nodes including that node is evaluated
- **THEN** the evaluation SHALL complete without a panic, and SHALL return each node once, in the same order on every run

#### Scenario: Nodes of different trees keep a stable order

- **GIVEN** two separate documents `d1:[x]` and `d2:[x]` whose nodes are combined in one expression
- **WHEN** a path that has to sort nodes of both trees is evaluated twice
- **THEN** both evaluations SHALL return the same order without an error

#### Scenario: Atomic results keep their order

- **GIVEN** the document `root:[section:[item(5),item(9),item(1)],section:[item(7)]]`
- **WHEN** `/root//item/xs:integer(value)` is evaluated
- **THEN** the result SHALL stay `[5, 9, 1, 7]`
- **NOTE:** Unchanged; atomic values are not sorted.

### Requirement: A positional predicate in a step counts per context node

A predicate within a step that uses `position()` or `last()`, or that yields a number, SHALL be evaluated for each context node separately. It SHALL count along the step's axis from that context node: in document order for forward axes, and in reverse document order for reverse axes. The step's result SHALL be the union of the per-context results, in document order. Predicates that are not positional SHALL select the same nodes as before.

#### Scenario: The first match of every parent

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** `//B[1]` and `count(//B[1])` are evaluated
- **THEN** the results SHALL be `[B1, B3, B4]` and `3`
- **NOTE:** Before this change `[B3]` and `1`.

#### Scenario: The last match of every parent

- **GIVEN** the same tree
- **WHEN** `//B[last()]` is evaluated
- **THEN** the result SHALL be `[B2, B3, B5]`
- **NOTE:** Before this change `[B5]`.

#### Scenario: A positional child step after a child step

- **GIVEN** the same tree
- **WHEN** `//P/B[2]` and `/W/*/B[1]` are evaluated
- **THEN** the results SHALL be `[B2, B5]` and `[B1, B4]`
- **NOTE:** Before this change `[B2]` and `[B1]`.

#### Scenario: A position after a filter counts the filtered nodes of each parent

- **GIVEN** the same tree
- **WHEN** `//B[@id!='B1'][1]` is evaluated
- **THEN** the result SHALL be `[B2, B3, B4]`
- **NOTE:** Before this change `[B3]`.

#### Scenario: Reverse axes count from each context node

- **GIVEN** the same tree
- **WHEN** `//B/ancestor::*[1]`, `//B/preceding-sibling::*[1]` and `//B/preceding::B[1]` are evaluated
- **THEN** the results SHALL be `[W, P1, P2]`, `[P1, B1, B4]` and `[B1, B2, B3, B4]`
- **NOTE:** Before this change `[W]`, `[P1]` and `[B2]`.

#### Scenario: Forward axes from several context nodes count from each

- **GIVEN** the same tree, the tree `r:[x1,x2,y1,x3,y2]`, and the tree `r:[X1:[B1,X2:[B2]]]`
- **WHEN** `//B/following::B[1]`, `//x/following-sibling::y[1]` and `//X/descendant::B[1]` are evaluated
- **THEN** the results SHALL be `[B2, B3, B4, B5]`, `[y1, y2]` and `[B1, B2]`
- **NOTE:** Before this change `[B4]`, one node, and one node. Context minimization may not merge context nodes when a predicate is positional.

#### Scenario: Positions that match nothing

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** `//B[3]`, `//B[0]` and `//B[1.5]` are evaluated
- **THEN** each result SHALL be empty, and no error SHALL be raised
- **NOTE:** `//B[3]` gives `[B2]` before this change.

#### Scenario: Non-positional predicates are unchanged

- **GIVEN** the same tree
- **WHEN** `//B[@id='B4']` and `//P[B]` are evaluated
- **THEN** the results SHALL be `[B4]` and `[P1, P2]`

#### Scenario: A predicate error is still raised

- **GIVEN** the same tree
- **WHEN** `//B[(1, 2)]` is evaluated
- **THEN** the evaluation SHALL fail with `FORG0006`

### Requirement: A predicate on a parenthesized expression counts over the whole sequence

A predicate on a parenthesized expression, `(E)[p]`, SHALL count `position()` and `last()` over the whole result of `E` in its order, independent of the steps inside `E`.

#### Scenario: The n-th match overall

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** `(//B)[1]`, `(//B)[2]`, `(//B)[last()]` and `(//P/B)[3]` are evaluated
- **THEN** the results SHALL be `B1`, `B2`, `B5` and `B4`
- **NOTE:** Before this change `B3`, `B1`, `B5` and `B4`.

#### Scenario: The first match overall at different depths

- **GIVEN** the tree `r:[A:[X1],X2]`
- **WHEN** `(//X)[1]` is evaluated
- **THEN** the result SHALL be `X1`
- **NOTE:** Before this change `X2`.

#### Scenario: A position past the end

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** `(//B)[6]` is evaluated
- **THEN** the result SHALL be empty

#### Scenario: Predicates over atomic sequences are unchanged

- **GIVEN** any context
- **WHEN** `(1 to 100)[position() > 3][position() <= 5][2]` is evaluated
- **THEN** the result SHALL be `5`

### Requirement: The first result does not require reading the rest of the tree

The first item of a streamed evaluation (the first item of the engine's stream, and what the runtime's `evaluate_single` returns) SHALL be the first item of the full result in document order. For the shapes `//T[p]`, `.//T[p]`, `A//T[p]`, `//A/T[p]`, `(//T[p])[1]`, `//T[n]`, `.//T[p][n]` and chains of child steps after them, finding the first item SHALL read only the lists of children of the item's ancestors, of the item itself, and of nodes that precede it in document order.

#### Scenario: The first match of a descendant search

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** the first item of `//B` is taken
- **THEN** it SHALL be `B1`
- **NOTE:** Before this change `B3`.

#### Scenario: The first match reads only what precedes it

- **GIVEN** a keyless tree of one `W` with 50 `P` children of 20 `B` children each (1,052 nodes), which records whose children were read
- **WHEN** the first item of `//B[@id='B0_3']`, `//B`, `(//B)[1]`, `//B[1]`, `//P/B[1]`, `//P/B[@id='B0_3']`, `//W/P[@id='P0']`, and of `.//B[2]` from `W` is taken
- **THEN** each SHALL be the first item of the full result, and every list read SHALL belong to that item, one of its ancestors, or a node before it in document order, while the full evaluation of `//B` reads 1,052 lists
- **NOTE:** A guard against sorting where streaming is possible.

#### Scenario: The runtime's first match reads only the lists up to it

- **GIVEN** the lazy fake provider of the runtime's lifetime tests (`root → 2 Window → [Pane, Button, Pane] → [Button, Text, Button] → [Button]`) without a retained snapshot
- **WHEN** the first item of the runtime's evaluation stream for `//Button` is taken, which is what `evaluate_single` returns
- **THEN** it SHALL be `root/0/0/0`, and only the lists of `root`, `root/0` and `root/0/0` SHALL have been read
- **NOTE:** Before this change it is `root/0/1`.

#### Scenario: A shape that has to sort still returns the first node in document order

- **GIVEN** the tree `W:[P1:[B1,B2],B3,P2:[B4,B5]]`
- **WHEN** the first item of `//B/..` is taken
- **THEN** it SHALL be `W`

#### Scenario: Cancellation stops a sort

- **GIVEN** a cancellation flag that is set while a path sorts its nodes
- **WHEN** the evaluation continues
- **THEN** it SHALL stop with the cancellation error, as streaming evaluations do
