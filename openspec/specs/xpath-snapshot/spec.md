# xpath-snapshot Specification

# Spec Delta

## Purpose

How the runtime answers XPath queries from a snapshot of the UI: one snapshot per query, which the next query may reuse and which the caller discards; how a snapshot that ends is released; and that a node the runtime hands out keeps its ancestors while it is held.

## Requirements

### Requirement: A query reads one snapshot that does not change while it runs

The runtime SHALL answer each query from a snapshot of the UI that does not change while the query runs, so that a query cannot break on a UI that changes under it. Within one query, the list of a node's children SHALL be read from the provider at most once, and every step of the query SHALL see that same list.

#### Scenario: Each list of children is read once per query

- **GIVEN** a provider whose tree is three to six levels deep
- **WHEN** queries such as `//Button`, `count(//*)`, `//Pane/following-sibling::*`, `(//Button)[last()]` and `//Button/preceding::Pane` run without a retained snapshot
- **THEN** for each query the provider SHALL be asked for the children of each node at most once
- **NOTE:** Exercised at the runtime unit level with a fake provider that counts its `children()` calls.

### Requirement: The next query may reuse the snapshot

A snapshot that the caller retains SHALL be reused by the next query from the same context node, which may search deeper in it. Before the next query, the runtime SHALL check each cached node it touches for validity, including the ancestors that a query from a held element reached: a list of children that holds a node that is no longer valid SHALL be read again, and a snapshot whose root is no longer valid SHALL be replaced. A node that was added under a parent whose cached children are all still valid SHALL NOT be seen until the caller discards the snapshot. After the caller has discarded it, the next query SHALL read the current UI.

#### Scenario: Repeating a query on a retained snapshot reads nothing new

- **GIVEN** a retained snapshot of a query whose nodes all stay valid
- **WHEN** the same query runs again
- **THEN** it SHALL return the same result without asking the provider for any children
- **NOTE:** Exercised at the runtime unit level with the counting fake provider.

#### Scenario: A node that is no longer valid is not returned

- **GIVEN** a retained snapshot that holds a node
- **WHEN** that node reports that it is no longer valid and the next query touches its parent's list of children
- **THEN** that list SHALL be read again from the provider, and the node SHALL NOT be part of the result

#### Scenario: A sibling that has gone is not returned from a held element

- **GIVEN** a retained snapshot of `following-sibling::*` evaluated from a held element, which returned two siblings
- **WHEN** one of the siblings reports that it is no longer valid and the same query runs again from the same element
- **THEN** the result SHALL contain only the sibling that is still valid
- **NOTE:** Exercised at the runtime unit level. Before this change the gone sibling was still returned, because the ancestors reached upward from a held element were never checked again.

#### Scenario: An added node appears only after the snapshot was discarded

- **GIVEN** a retained snapshot, and a node added in the UI under a parent whose cached children are all still valid
- **WHEN** the next query runs, and then another query runs after the caller discarded the snapshot
- **THEN** the first query SHALL NOT see the added node, and the second SHALL see it
- **NOTE:** Exercised at the runtime unit level. On Windows it was also observed against a real application: the first window of a newly started process appeared only after `Runtime.clear_cache()`.

### Requirement: A snapshot that ends is released

When a snapshot ends, the runtime SHALL release it, together with every provider node that only the snapshot holds. A snapshot ends when the caller discards it, when a query from another context node replaces it, when a query without a retained snapshot has returned and its results are dropped, when a stream of results is dropped before it is exhausted, when a query fails, and when the runtime shuts down. A retained snapshot SHALL NOT grow when revalidation reads a list of children again. Releasing a deep snapshot SHALL NOT overflow the stack. A provider that panics during a query SHALL make that query fail with the panic; releasing the snapshot afterwards SHALL NOT abort the process.

#### Scenario: A discarded snapshot is released

- **GIVEN** a retained snapshot of `//Button`
- **WHEN** the caller discards it
- **THEN** the number of live provider nodes SHALL return to what it was before the query
- **NOTE:** Exercised at the runtime unit level with a fake provider that counts created and dropped nodes. The mock provider owns its whole tree and cannot show a release.

#### Scenario: A query without a retained snapshot leaves nothing behind

- **GIVEN** no retained snapshot
- **WHEN** `count(//*)` runs, and when `evaluate_single` runs and its result is dropped
- **THEN** the number of live provider nodes SHALL return to what it was before the query

#### Scenario: A snapshot replaced by another context is released

- **GIVEN** a retained snapshot built from one window as context
- **WHEN** a query runs from another window as context
- **THEN** only the second window's snapshot SHALL remain live

#### Scenario: A stream dropped early is released

- **GIVEN** a stream of query results
- **WHEN** it is dropped after two items
- **THEN** the number of live provider nodes SHALL return to what it was before the query

#### Scenario: A query that fails is released

- **GIVEN** a query that fails to compile, with and without a retained snapshot
- **WHEN** it returns its error
- **THEN** the number of live provider nodes SHALL return to what it was before the query

#### Scenario: Revalidation does not grow a retained snapshot

- **GIVEN** a retained snapshot whose containers turn invalid before each of three queries
- **WHEN** the three queries run
- **THEN** each SHALL return the correct result, the number of live provider nodes SHALL NOT grow from one query to the next, and discarding the snapshot SHALL return it to what it was before the first query

#### Scenario: A deep snapshot is released without overflowing the stack

- **GIVEN** a snapshot of a chain 10,000 levels deep whose nodes keep their parents, built on a thread with a large stack
- **WHEN** the snapshot is discarded on a thread with a 256 KiB stack
- **THEN** it SHALL be released without a stack overflow, in debug and in release builds
- **NOTE:** Exercised at the runtime unit level. Release must not recurse per level, neither through the snapshot nor through the providers' chains of parents.

#### Scenario: A provider that panics does not abort the process

- **GIVEN** a retained snapshot, and a provider whose validity check, or whose list of children while it is read, panics during the next query
- **WHEN** the query runs, and the snapshot is discarded afterwards
- **THEN** the query SHALL fail with the panic, which a caller can catch, and discarding the snapshot SHALL NOT abort the process
- **NOTE:** Exercised at the runtime unit level with `catch_unwind`. A list that panics while it is read poisons the snapshot's lock of that list, so the case also shows that the snapshot still answers the next query.

#### Scenario: Shutting down releases the snapshot while the providers still run

- **GIVEN** a runtime with a retained snapshot
- **WHEN** the runtime shuts down
- **THEN** the snapshot SHALL be released before the providers are shut down

#### Scenario: Discarding snapshots does not grow memory on a real desktop

- **GIVEN** the Windows desktop with a large application window, such as an editor with thousands of elements
- **WHEN** a query under that window runs 20 times from Python, with `clear_cache()` before each run
- **THEN** the process's private memory SHALL NOT grow in proportion to the number of runs
- **NOTE:** Verifiable only against a real provider, as a manual measurement on Windows that is recorded in the change's tasks. Before this change it grew by about 15 MiB per run.

### Requirement: A node that is handed out keeps its ancestors while it is held

A node that the runtime hands out — as a query result with or without a retained snapshot, as a child of a held node, or as a hit-test result — SHALL keep its chain of ancestors reachable for as long as the node is held, after its snapshot has been released. It SHALL keep only that chain, not the rest of its snapshot. Once the node is dropped, its ancestors SHALL be released too, unless something else holds them. The runtime's desktop node is the end of every chain and lives as long as its runtime.

#### Scenario: A result keeps its ancestors after its snapshot was discarded

- **GIVEN** a button below a pane, returned from a retained snapshot
- **WHEN** the snapshot is discarded
- **THEN** the node's ancestors SHALL still reach its window, `count(ancestor::*)` evaluated from the node SHALL count every ancestor element up to its window, and only that chain SHALL remain live besides the node
- **NOTE:** Exercised at the runtime unit level with a fake provider that follows the provider rule of the next requirement.

#### Scenario: A result from a query without a retained snapshot can activate its window

- **GIVEN** a button returned by a query without a retained snapshot, as `platynui-cli pointer` runs it
- **WHEN** the button's window is brought to the front
- **THEN** the window SHALL be activated
- **NOTE:** Exercised at the runtime unit level with a fake window that counts activations.

#### Scenario: A captured element still raises its window after the snapshot was discarded

- **GIVEN** a BareMetal suite with two windows A and B at the same position, that captures a button in window B with `Query    …    only_first=${True}`, then runs another `Query`, which discards the snapshot, and brings window A to the front so that it covers B
- **WHEN** the suite clicks the captured button
- **THEN** window B SHALL be brought to the front, the click SHALL reach the button, the button's bounds SHALL be the same as before the snapshot was discarded, and the button SHALL still reach its window through its ancestors
- **NOTE:** Verifiable only against a real provider. Covered on every lane with the egui test app (UI Automation on Windows, AT-SPI on X11 and Wayland), and on the Windows lane with Swing through the Java Access Bridge and the Java agent, under a root pinned to the application node. UI Automation can raise a window through any of its elements, so on Windows the raise alone does not show that the ancestors were kept; the last check does.

#### Scenario: A root inside a window still activates that window

- **GIVEN** a BareMetal suite with two windows A and B at the same position, whose scoped root is the container of a button in window B (a node whose role is not `Window`), while window A covers B
- **WHEN** the suite clicks the button below that root
- **THEN** window B SHALL be brought to the front and the click SHALL reach the button
- **NOTE:** Verifiable only against a real provider, on every lane with the egui test app, whose button row is an accessible group for this purpose (AccessKit drops egui's plain containers). Both acceptance scenarios are confirmed once to fail on a build that releases snapshots but does not keep parents.

### Requirement: A provider keeps the parent of every node it lists

A provider SHALL keep the parent of each node it lists as a child reachable for as long as that node lives. It SHALL NOT make a node hold its own children, and it SHALL NOT keep the runtime's desktop node alive, which its top-level nodes reach only while the runtime lives. A provider that owns its whole tree, such as the mock provider, meets this requirement already.

#### Scenario: A child's parent survives when everything else is dropped

- **GIVEN** a node of a provider, held by nobody but the check
- **WHEN** its children are listed, and every other reference to the node and to the listing is dropped
- **THEN** each child's parent SHALL still be reachable and SHALL be the node the children were listed from
- **NOTE:** Checked by the core contract testkit, against UI Automation in a unit test that runs on every Windows `just test`, and against the Java Access Bridge and the Java agent in the live Java tests of the Windows lane. AT-SPI is covered by the acceptance scenarios above, which fail without the rule.

#### Scenario: Nodes never hold their children

- **GIVEN** a node of a provider and the nodes of its subtree, listed down to a bounded number
- **WHEN** every reference the check itself took is dropped
- **THEN** every listed node SHALL have been released
- **NOTE:** Checked by the core contract testkit, with the same providers as above. It fails for a provider that caches child nodes, which together with the rule above would bring back a cycle. The mock provider owns its tree and is exempt.

#### Scenario: A provider that does not keep parents is reported

- **GIVEN** a provider whose nodes keep their parent only weakly
- **WHEN** the contract testkit checks it
- **THEN** the check SHALL report each child whose parent is no longer reachable

#### Scenario: A check that cannot prove anything says so

- **GIVEN** a node that the caller of the check still holds elsewhere
- **WHEN** the contract testkit checks that its children keep it
- **THEN** the check SHALL report that it cannot prove the rule, instead of passing
