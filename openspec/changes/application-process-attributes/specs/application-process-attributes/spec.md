# Spec Delta

## Purpose

The process attributes of an application node, for every provider that builds one: which attributes exist and where they live, the one value format each has, the rule that an attribute is reported only when it was determined for that process and never substituted, which platform supplies which attribute, and that a process ID of `0` identifies nothing. Platforms and providers added later implement against this contract instead of inventing their own.

## ADDED Requirements

### Requirement: Process attributes have fixed names and namespaces

An application node SHALL report its process attributes under these names and namespaces, and under no other:

- `ProcessId` in the `control` namespace — addressed as `@ProcessId`, the form every window lookup and every suite uses.
- `ProcessName`, `ExecutablePath`, `CommandLine`, `UserName`, `StartTime` and `Architecture` in the `app` namespace — addressed as `@app:ProcessName` and so on.

A provider SHALL NOT report one of these facts under a second name or namespace as well. A node's display name (`control:Name`) and developer id (`control:Id`) are not process attributes. Where a provider names an application node after its program — UI Automation and the Java Access Bridge do — that name SHALL be the node's `ProcessName`, and a `control:Id` that carries the name SHALL carry the same value.

#### Scenario: An application named after its program carries its process name

- **GIVEN** an application node from UI Automation or the Java Access Bridge for a process whose executable is `C:\Tools\notepad.exe`
- **WHEN** `@Name` and `@app:ProcessName` are read
- **THEN** both SHALL be `notepad`, so that `app:Application[@Name="notepad"]` and `app:Application[@app:ProcessName="notepad"]` select the same node
- **NOTE** Real provider only, Windows. Today both providers compute the name on their own, from the executable's stem. For a `.exe` program the two values already agree; for an image with another extension they can differ.

#### Scenario: A Java application served by the in-JVM agent carries its process attributes under app

- **GIVEN** a Java application whose application node is built from the in-JVM agent
- **WHEN** `@app:ProcessName` and `@ProcessName` are read on that node
- **THEN** `@app:ProcessName` SHALL be present and `@ProcessName` SHALL be absent
- **NOTE** Real provider only. Today the agent reports these attributes in the `control` namespace, so the `app` form finds nothing on a Java application while it works on every other provider.

#### Scenario: The process ID stays addressable in the control namespace

- **GIVEN** an application node from any provider whose process ID is known
- **WHEN** `/app:Application[@ProcessId=N]` is evaluated with that process ID
- **THEN** exactly that application node SHALL be selected

### Requirement: A process attribute is reported only when it was determined, never substituted

Every process attribute SHALL be optional, `ProcessId` included. An attribute SHALL be present only when its value was actually determined for that process, from a source that describes that process. Enumerating a node's attributes and testing for one by name SHALL agree: `[@app:CommandLine]` is true exactly when the command line is listed.

An attribute that cannot be determined SHALL be absent. It SHALL NOT be answered with a substituted value: not an empty string, a null, `"unknown"` or `0`, and not a value that describes the automation host or another process instead of the application — the host's or the build's architecture, the account the automation runs as, or the like. A plausible wrong answer is worse than a missing one, because nothing distinguishes it from a real one. The presence of one process attribute SHALL NOT imply the presence of another.

#### Scenario: An unreadable process carries no substituted attributes

- **GIVEN** an application whose process the runtime is not permitted to query, for example a process of another user or an elevated process on Windows
- **WHEN** its application node's attributes are listed
- **THEN** every process attribute that could not be read SHALL be missing from the listing, and none SHALL be present with an empty string, a null, `"unknown"` or `0`
- **NOTE** Real provider only. Today the Windows UIA provider lists all seven attributes for such a process and answers `""`, a null or `"unknown"`. The JAB provider answers a null, an empty executable path, the image name with `.exe` as the process name, and `"unknown"` for an architecture its PE parser does not know.

#### Scenario: Listing and predicate agree for a missing attribute

- **GIVEN** an application node whose command line cannot be read while its process name can
- **WHEN** the node's attributes are listed, and `[@app:CommandLine]` and `[@app:ProcessName]` are evaluated on it
- **THEN** the listing SHALL contain `ProcessName` but not `CommandLine`, `[@app:CommandLine]` SHALL be false, and `[@app:ProcessName]` SHALL be true
- **NOTE** Verifiable with the mock once its application nodes model process attributes; otherwise real provider only.

#### Scenario: An architecture that cannot be read is absent, not the host's

- **GIVEN** an application on a platform that reports architectures, whose architecture the provider cannot determine
- **WHEN** `@app:Architecture` is read
- **THEN** it SHALL be absent, and SHALL NOT be the architecture of the machine the runtime runs on or the one the runtime was built for
- **NOTE** Real provider only. Today the Windows UIA provider falls back to the machine's architecture and the JAB provider to the one the runtime was built for — both wrong for a 32-bit process on 64-bit Windows.

#### Scenario: An application's self-description does not replace a process attribute

- **GIVEN** a Java application started with `-Duser.name=someone-else` under the account `A`
- **WHEN** `@app:UserName` is read on its application node
- **THEN** it SHALL name the account `A`, never `someone-else`
- **NOTE** Real provider only. Process attributes describe the process as the platform knows it; a value an application states about itself is not a source for them.

### Requirement: A process ID of 0 identifies nothing

`ProcessId` SHALL NEVER be `0` or negative. Where the platform answers `0` or nothing, the attribute SHALL be absent. Wherever a window and an application are correlated through a process ID — finding an application node's windows or popups, activating, moving or closing its window, resolving the application behind a window at a point — a process ID of `0` SHALL NOT be used: the correlation SHALL report that nothing was resolved.

#### Scenario: A platform that reports process ID 0 yields no process ID

- **GIVEN** an element whose platform reports the process ID `0`, for example an element found at a point whose process the platform cannot name
- **WHEN** the application node built for it is read
- **THEN** that node SHALL carry no `ProcessId` attribute
- **NOTE** Real provider only. Today the Windows UIA provider builds such an application node with the process ID `0`.

#### Scenario: A window is never looked up by process ID 0

- **GIVEN** a node that carries `ProcessId = 0`, for example from a provider that does not follow this contract
- **WHEN** a window-management operation resolves the node's window
- **THEN** it SHALL report that no window was resolved, and SHALL NOT act on any window found by looking up the process ID `0`
- **NOTE** Decidable per window manager with an injected node, without a real desktop. Today the window managers on Windows, X11 and the PlatynUI compositor accept an integer `0` and look windows up by it.

### Requirement: Each process attribute has one value format

A present process attribute SHALL have exactly this form, whichever provider reports it:

- **`ProcessId`**: a positive integer.
- **`ProcessName`**: the file name of the executable the process runs, without its directory and without the extension the platform gives executables (`.exe` on Windows). Examples: `notepad`, `javaw`, `python3`.
- **`ExecutablePath`**: the absolute path of that executable, as the process's own platform presents it.
- **`CommandLine`**: one string. On Windows it is the command line the process was started with, verbatim, quoting included. On Linux and macOS it is the process's arguments joined by single spaces, the way `ps` shows them.
- **`UserName`**: the account the process runs as, in the platform's form. On Windows that is `DOMAIN\user`, with the computer name as the domain for a local account. On Linux and macOS it is the login name.
- **`StartTime`**: the moment the process was created, as an ISO-8601 UTC string to the second, `YYYY-MM-DDTHH:MM:SSZ`, without fractional seconds.
- **`Architecture`**: one of `x86`, `x64`, `arm`, `arm64`. It names the architecture the process's code runs as: a 32-bit process on a 64-bit system reports its 32-bit architecture, and a process the platform runs under emulation reports the emulated architecture. Windows classifies an ARM64EC process as x64-compatible, and it reports `x64`. An architecture outside this list, or one the platform cannot tell, is absent. The list is extended by a change to this specification when a supported platform needs another value.

#### Scenario: The start time has one format on every provider

- **GIVEN** an application node from any provider whose start time is known
- **WHEN** `@app:StartTime` is read
- **THEN** it SHALL match `YYYY-MM-DDTHH:MM:SSZ` exactly, and SHALL equal the process's creation time in UTC truncated to the second
- **NOTE** Real provider per platform. Today the forms differ: Windows UIA adds milliseconds, JAB and AT-SPI stop at the second, and the Java agent reports an integer of epoch milliseconds.

#### Scenario: A Windows process owned by a local account names the computer as its domain

- **GIVEN** an application on Windows running under a local account `user` on the computer `HOST`
- **WHEN** `@app:UserName` is read
- **THEN** it SHALL be `HOST\user`
- **NOTE** Real provider only, Windows. Today the JAB provider reports the bare `user`.

#### Scenario: A Windows command line keeps its quoting

- **GIVEN** an application on Windows started as `app.exe "C:\My Files\input.txt" --flag`
- **WHEN** `@app:CommandLine` is read
- **THEN** it SHALL contain `"C:\My Files\input.txt"` with its quotes, exactly as the process was started
- **NOTE** Real provider only, Windows. Today the JAB provider rebuilds the line from its arguments and loses the quoting.

#### Scenario: A 32-bit process on 64-bit Windows reports its own architecture

- **GIVEN** a 32-bit application running on 64-bit Windows
- **WHEN** `@app:Architecture` is read
- **THEN** it SHALL be `x86`
- **NOTE** Real provider only, Windows, and only where a 32-bit application is available to the lane.

#### Scenario: A Java application reports its process, not its main class

- **GIVEN** a Java application whose application node is built from the in-JVM agent, launched through `javaw.exe` with the main class `com.example.App`
- **WHEN** `@app:ProcessName` and `@app:ExecutablePath` are read
- **THEN** `@app:ProcessName` SHALL be `javaw` and `@app:ExecutablePath` SHALL be the path of that `javaw.exe`, while the main class MAY remain the node's display name
- **NOTE** Real provider only. Today the agent reports the main class's simple name, or the jar's file name, as the process name, and `<java.home>\bin\java.exe` as the executable path, also for a JVM started through `javaw.exe`.

### Requirement: Each platform reports the process attributes it has a source for

A provider SHALL report a process attribute only where its platform has a source that describes the process itself, and every provider on one platform SHALL report the same value for the same process:

- **Windows** supplies all seven attributes.
- **Linux** supplies `ProcessId`, `ProcessName`, `ExecutablePath`, `CommandLine`, `UserName` and `StartTime`. It does not supply `Architecture`: Linux keeps no architecture per process. Which process ID an AT-SPI application reports, and when the process table may be read at all, is specified by `sidecar-deployment`.
- **macOS** supplies none today, because its provider builds no application nodes. When it does, it reports the attributes its platform has a source for, under this specification.

#### Scenario: A Linux application carries no architecture

- **GIVEN** an application on Linux whose process the runtime can read
- **WHEN** its application node's attributes are listed
- **THEN** `ProcessName`, `ExecutablePath`, `CommandLine`, `UserName` and `StartTime` SHALL be present, and `Architecture` SHALL be absent
- **NOTE** Real provider only, Linux.

#### Scenario: Two providers report the same process identically

- **GIVEN** one process whose application node can be built by two providers on the same platform, for example a Java application reached through JAB and through the in-JVM agent on Windows
- **WHEN** the process attributes of both nodes are read
- **THEN** every attribute present on both SHALL have the same value on both
- **NOTE** Real provider only, Windows.

### Requirement: The Python Application object reads the process attributes

`PlatynUI.ui.Application` SHALL read its process from the attributes of this specification: `process_id` from `control:ProcessId`, and `process_name` from `app:ProcessName`. Each SHALL answer `None` when its attribute is absent, and SHALL raise `TypeError` only when the attribute is present with a value of the wrong type.

#### Scenario: The process ID and name of an application are read

- **GIVEN** an application node that carries `control:ProcessId` and `app:ProcessName`, such as the mock's "Mock Application"
- **WHEN** `process_id` and `process_name` of its `Application` are read
- **THEN** they SHALL return the node's process ID and process name
- **NOTE** Verifiable against the mock once its application models process attributes. Today `process_id` reads `app:ProcessId`, which no provider reports, and always fails.

#### Scenario: A missing attribute gives None

- **GIVEN** an application node without process attributes, such as the mock's "Mock Settings"
- **WHEN** `process_id` and `process_name` of its `Application` are read
- **THEN** both SHALL return `None`

#### Scenario: An agent-served Java application reports its process name

- **GIVEN** a Java application whose application node is built from the in-JVM agent
- **WHEN** `process_name` of its `Application` is read
- **THEN** it SHALL return the JVM process's name, for example `javaw`
- **NOTE** Real provider only, Windows. Today it raises `TypeError`, because the agent reports the name under `control`.
