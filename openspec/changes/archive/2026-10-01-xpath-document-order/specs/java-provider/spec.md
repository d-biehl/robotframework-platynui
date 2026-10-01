# Spec Delta

## ADDED Requirements

### Requirement: Agent runtime ids are scoped per view

The agent backend shows a Java window twice: once in the desktop's flat list of windows, and once under its process's `app:Application` node. Each view SHALL be a separate node with its own runtime id, as for UI Automation and the Java Access Bridge. The same element SHALL carry `agent/<pid>/<element id>` in the flat view and `agent/app/<pid>/<element id>` in the application view. A node SHALL take the view of the node it was listed from, down to every descendant. A hit-test result SHALL belong to the application view, because its ancestors lead to the `app:Application` node. The ids in `SelectedItems` SHALL be those of the child nodes in the same view as the node that reports them. Within one view, the runtime id of an element SHALL stay the same across enumerations and across PlatynUI hosts that share the agent. (Real-provider-only: needs a JVM with the agent loaded; runs against the Swing fixture.)

#### Scenario: The two views of a window are two nodes

- **GIVEN** the Swing fixture, served by the agent, with the title `<title>`
- **WHEN** `(/control:Window[@Name="<title>"][@Technology="JavaAgent"] | /app:Application/control:Window[@Name="<title>"][@Technology="JavaAgent"])` is evaluated
- **THEN** the result SHALL hold two nodes, with runtime ids of the form `agent/<pid>/<element id>` and `agent/app/<pid>/<element id>` for the same element id
- **NOTE:** Before this change the result holds one node, because both carry the same runtime id.

#### Scenario: Descendants carry their window's view

- **GIVEN** the same fixture
- **WHEN** the same button is reached below the flat window and below the window under the `app:Application` node
- **THEN** its runtime ids SHALL start with `agent/<pid>/` and `agent/app/<pid>/`, and SHALL differ only in that prefix

#### Scenario: An element keeps its id within a view

- **GIVEN** the same fixture, and two provider instances that share the fixture's agent
- **WHEN** both enumerate the flat window, and each enumerates the table's rows twice
- **THEN** both SHALL report the same runtime id for the window, and each row SHALL keep its runtime id across the enumerations

#### Scenario: SelectedItems names nodes of its own view

- **GIVEN** the fixture's table with its third row selected
- **WHEN** `SelectedItems` is read from the table below the flat window and from the table below the window under the `app:Application` node
- **THEN** each SHALL name exactly the runtime id of the third row in the same view

#### Scenario: A hit-test result lies in the application view

- **GIVEN** a point inside a button of the fixture
- **WHEN** the backend resolves the element at that point
- **THEN** the result's runtime id SHALL equal that of the same button reached through the `app:Application` node, and its ancestors SHALL lead to that node
