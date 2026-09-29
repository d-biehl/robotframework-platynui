# atspi-application-level Specification

## Purpose

Defines which AT-SPI accessibility objects PlatynUI exposes as application-level nodes, which namespace and role they carry, and what the AT-SPI Application interface does and does not decide for the nodes below that level.

## Requirements

### Requirement: The registry's applications are the application level

The AT-SPI provider SHALL expose every application that the accessibility registry lists as an application-level node in the `app` namespace, directly under the desktop. Such an application is the root object an application registered through `Socket.Embed`. Whether that object reports the `org.a11y.atspi.Application` interface SHALL NOT affect this. The point hit-test SHALL classify the application it resolves the same way as enumeration does.

#### Scenario: A registered application is an application-level node

- **GIVEN** the Qt test application is registered on the accessibility bus
- **WHEN** the desktop's children are enumerated
- **THEN** its registry root is a node in the `app` namespace directly under the desktop
- **AND** no node below it is in the `app` namespace
- **NOTE** Verifiable only against a real provider.

#### Scenario: A registered application without the Application interface stays at the application level

- **GIVEN** a registered application whose root object does not report the Application interface
- **WHEN** it is classified as a child of the registry
- **THEN** its node is in the `app` namespace
- **NOTE** No real toolkit is known to do this. It is verified on the provider's classification in isolation.

#### Scenario: The hit-test reaches the window through one application node

- **GIVEN** the egui test application's window is at a point on the screen
- **WHEN** the element at that point is resolved
- **THEN** exactly one node in the result's ancestor chain is in the `app` namespace, the application's registry root
- **AND** the window's frame is its direct child, in the `control` namespace
- **NOTE** Verifiable only against a real provider with a window manager.

### Requirement: An application-level node carries the role its object reports

The role of an application-level node SHALL be the role its object reports through the Accessible interface, mapped the same way as the role of every other node. The provider SHALL NOT replace it with `Application`. Only the namespace SHALL be fixed by the level.

#### Scenario: A root with the application role is app:Application

- **GIVEN** a registered application whose root object reports the role `application`, as GTK 4, Qt 6, AccessKit and Avalonia 12 do
- **WHEN** it is classified
- **THEN** its node is `app:Application`
- **AND** `native:Accessible.Role` is `application`

#### Scenario: A root with another role keeps that role

- **GIVEN** a registered application whose root object reports the role `frame`
- **WHEN** it is classified
- **THEN** its node is `app:Frame`
- **NOTE** No real toolkit is known to do this. It is verified on the provider's classification in isolation.

#### Scenario: A root whose role cannot be read stays at the application level

- **GIVEN** a registered application whose `Accessible.GetRole` call does not answer in time
- **WHEN** the desktop's children are enumerated
- **THEN** its node is `app:Unknown`
- **AND** it is neither left out nor placed in another namespace
- **NOTE** It is verified on the provider's classification in isolation, with the role the enumeration substitutes for an unanswered call.

### Requirement: The Application interface does not classify a node below the application level

Below the application level, a node's namespace and role SHALL follow from its Accessible role alone. A node there that reports the Application interface SHALL NOT be placed in the `app` namespace. It SHALL NOT carry `@ProcessId` or any `app:*` process attribute, and its `Id` SHALL NOT be derived from a process ID. Its interfaces and the properties of its Application interface SHALL remain visible as native attributes: `native:Accessible.Interfaces` and `native:Application.*`.

#### Scenario: An Avalonia window is a frame

- **GIVEN** a running Avalonia 12 application, whose window object reports the Application interface and the role `frame`
- **WHEN** the children of its application node are enumerated
- **THEN** the window is `control:Frame`
- **AND** it has no `@ProcessId` and no `app:*` attribute
- **AND** `native:Accessible.Interfaces` contains `Application`
- **NOTE** Verifiable only against a real Avalonia application. The classification is also covered by a unit test.

#### Scenario: Only the children of the application level are top-level windows

- **GIVEN** the same Avalonia application
- **WHEN** the supported patterns of its window and of the window's children are read
- **THEN** the window exposes `Activatable` and the other window patterns
- **AND** none of the window's children exposes a window pattern or `@IsActive`
- **NOTE** Verifiable only against a real Avalonia application.

#### Scenario: Process attributes and Id follow the level, not the interface

- **GIVEN** a node below the application level that reports the Application interface and whose bus peer has a known process ID
- **WHEN** its attributes and its `Id` are read
- **THEN** it has no `@ProcessId` and no `app:*` attribute
- **AND** its `Id` is not that process ID
- **NOTE** It is verified on the provider's decision in isolation.

### Requirement: A node below the application level with the application role is a control

A node below the application level whose Accessible role is `application` SHALL be exposed as `control:Application`.

#### Scenario: An application role inside a tree is a control

- **GIVEN** a node below the application level whose role is `application`
- **WHEN** it is classified
- **THEN** its node is `control:Application`
- **NOTE** No fixture exposes such a node. It is verified on the provider's classification in isolation.

### Requirement: Consumers recognise an application node by its namespace

A component that needs the application node of a tree SHALL recognise it by the `app` namespace alone, and SHALL NOT require the role `Application`. One such component is the runtime, when it resolves the window to activate for an element.

#### Scenario: An application node with another role still leads to its window

- **GIVEN** a node in the `app` namespace whose role is `Frame`, with a child that exposes `Activatable`
- **WHEN** the runtime resolves the top-level window for that node
- **THEN** it returns that child

#### Scenario: A control named Application does not lead to a window

- **GIVEN** a `control:Application` node without an `Activatable` ancestor, with a child that exposes `Activatable`
- **WHEN** the runtime resolves the top-level window for that node
- **THEN** it finds no window
