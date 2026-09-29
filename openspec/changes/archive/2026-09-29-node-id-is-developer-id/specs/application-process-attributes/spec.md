# Spec Delta

## MODIFIED Requirements

### Requirement: Process attributes have fixed names and namespaces

An application node SHALL report its process attributes under these names and namespaces, and under no other:

- `ProcessId` in the `control` namespace — addressed as `@ProcessId`, the form every window lookup and every suite uses.
- `ProcessName`, `ExecutablePath`, `CommandLine`, `UserName`, `StartTime` and `Architecture` in the `app` namespace — addressed as `@app:ProcessName` and so on.

A provider SHALL NOT report one of these facts under a second name or namespace as well. A node's display name (`control:Name`) and developer id (`control:Id`) are not process attributes. Where a provider names an application node after its program — UI Automation and the Java Access Bridge do — that name SHALL be the node's `ProcessName`. An application node carries no `control:Id` at all (capability `id-attribute`).

#### Scenario: An application named after its program carries its process name

- **GIVEN** an application node from UI Automation or the Java Access Bridge for a process whose executable is `C:\Tools\ledger.exe`
- **WHEN** `@Name` and `@app:ProcessName` are read
- **THEN** both SHALL be `ledger`, so that `app:Application[@Name="ledger"]` and `app:Application[@app:ProcessName="ledger"]` select the same node
- **NOTE** Real provider only, Windows. Today both providers derive the name and `@app:ProcessName` from the executable's stem, so the two agree. An image with an extension other than `.exe` loses that extension, and JAB under limited rights carries `.exe` in both. Under this contract only `.exe` is removed, and the name follows the process name.

#### Scenario: A Java application served by the in-JVM agent carries its process attributes under app

- **GIVEN** a Java application whose application node is built from the in-JVM agent
- **WHEN** `@app:ProcessName` and `@ProcessName` are read on that node
- **THEN** `@app:ProcessName` SHALL be present and `@ProcessName` SHALL be absent
- **NOTE** Real provider only. Today the agent reports these attributes in the `control` namespace, so the `app` form finds nothing on a Java application while it works on every other provider.

#### Scenario: The process ID stays addressable in the control namespace

- **GIVEN** an application node from any provider whose process ID is known
- **WHEN** `/app:Application[@ProcessId=N]` is evaluated with that process ID
- **THEN** exactly that application node SHALL be selected
