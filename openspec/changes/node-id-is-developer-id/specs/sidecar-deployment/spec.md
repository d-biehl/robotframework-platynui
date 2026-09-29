# Spec Delta

## REMOVED Requirements

### Requirement: An application reports its own process ID, and process-table data only through a process ID valid in the runtime's namespace

**Reason**: Its last paragraph and two of its scenarios made the node identifier of an application follow its process ID. The capability `id-attribute` now states that an application node carries no `Id` on any provider, and OpenSpec cannot drop scenarios from a modified requirement.

**Migration**: The requirement continues unchanged, apart from the node identifier, as *An application reports its own process ID, and reads process-table data only through a process ID valid in the runtime's namespace* below. Consumers that read `element.id` of an application for its process ID read `@ProcessId` instead; in the Python library, `Application.process_id` reads that attribute.

## ADDED Requirements

### Requirement: An application reports its own process ID, and reads process-table data only through a process ID valid in the runtime's namespace

An application node SHALL report as its process ID the number the application's own environment knows it by — the process ID the accessibility bus daemon reports for that application's connection — and SHALL read process-table data only through a process ID valid in the runtime's own namespace. Whether the reported number *is* such a process ID is decided once per connection rather than per application: it is one exactly under *local numbering*, where the daemon has shown that it numbers processes as the runtime does. Under *no identity* the provider has no process ID valid in its own namespace for any application on that bus, whatever the reported number looks like. In a shared namespace the two coincide, which is why an ordinary desktop sees no change. The provider SHALL NOT compare a reported process ID with one from another namespace; reporting a process ID as an attribute is not a comparison.

The process-ID attribute of an application node (`@ProcessId` on `app:Application`) is the application's identity. It SHALL report the process ID as the application's own environment knows it: the process ID the accessibility bus daemon reports for that application's connection. It SHALL be reported whether or not that number is valid in the runtime's own namespace. It SHALL be **absent** when the daemon cannot tell — it omits the process ID, reports `0`, answers that the process ID is unknown, or the lookup fails — and it SHALL NEVER be `0`.

Every attribute read from the local process table — process name, executable path, command line, user name, start time — SHALL be read only through a process ID valid in the runtime's own namespace, and SHALL be **absent** when the application has none there. The provider SHALL NOT read the local process table with a number valid only in another namespace, and SHALL NOT guess.

A locally valid process ID is a precondition for those attributes, not a guarantee of them: each process-table attribute SHALL be reported only when its value was actually read for that process, and SHALL be absent otherwise. In particular, an attribute the provider cannot determine SHALL NOT be answered with a substituted value — an empty string, a placeholder, or a value that describes the automation host instead of the application; a plausible wrong answer is worse than a missing one, because nothing distinguishes it from a real one. The presence of the process-ID attribute SHALL NOT imply the presence of the process-table attributes.

Whether an application node carries a node identifier (`Id`) is stated by the capability `id-attribute`, for every provider.

#### Scenario: The application's own process ID is reported across the namespace boundary

- **GIVEN** the sidecar topology, and an application for whose connection the bus daemon reports the process ID `N`, where `N` is not the application's process ID in the runtime's namespace
- **WHEN** that application node's attributes are read
- **THEN** its process-ID attribute SHALL be `N` — the number the application's own container shows for it
- **NOTE** Decidable from injected credentials at the level of what the node reports; end to end only against a real provider across two PID namespaces, not the mock.

#### Scenario: An application the daemon cannot tell about carries no process ID, and never 0

- **GIVEN** the sidecar topology, and an application whose connection the bus daemon cannot resolve for us, in each of the three ways daemons answer: the credentials carry the process ID `0` (dbus-daemon 1.12/1.14, dbus-broker 29/33); the credentials omit it while the dedicated process-ID query answers a successful `0` (dbus-broker 35/37); the credentials omit it while that query answers that the process ID is unknown (dbus-daemon ≥ 1.15.10)
- **WHEN** that application node's attributes are read
- **THEN** it SHALL carry no process-ID attribute — in particular not one with the value `0`
- **NOTE** Decidable from injected credentials for all three shapes; end to end against a real daemon only for the implementations installed. Measured today with three applications on one bus: on dbus-daemon 1.14.10 and dbus-broker 37 such an application reports the process ID `0`, on dbus-daemon 1.16.2 no process-ID attribute at all — the two behave differently for anything keying off the attribute.

#### Scenario: Process-table attributes need a local process ID even when the process ID is reported

- **GIVEN** the sidecar topology, where the bus daemon cannot see the runtime and the outcome is therefore *no identity*, and an application whose process ID the daemon does report
- **WHEN** that application node's attributes are read
- **THEN** the process-ID attribute SHALL be present, and every process-table attribute SHALL be absent
- **NOTE** Decidable from injected inputs; end to end only against a real provider across two PID namespaces. This is the normal picture of a sidecar deployment. Measured today: the process-table attributes are present and empty or wrong there, because they are read with the reported number.

#### Scenario: An attribute that cannot be determined is absent, not guessed

- **GIVEN** an application whose process the provider cannot read
- **WHEN** its process-table attributes are read
- **THEN** every such attribute SHALL be absent, and none SHALL report a placeholder or a value taken from the automation host instead of the application
- **NOTE** Real provider only. Measured before this change: the architecture attribute fell back to the architecture the runtime was built for, so an application the runtime could not even see was reported as `x64` — indistinguishable from a real answer, and wrong outright on a container of another architecture. The provider no longer reports an architecture at all (design D7).

#### Scenario: A local process ID does not promise process-table attributes

- **GIVEN** an application with a process ID valid in the runtime's namespace, while the runtime cannot read some of that process's entries in the local process table
- **WHEN** that application node's attributes are read
- **THEN** the attributes that could not be read SHALL be absent rather than empty or substituted
- **NOTE** Real provider only. Not measured as such: the empty values measured in the sidecar came from reading the process table with a number that was not local, which the previous scenarios cover. Today an unreadable value is answered with an empty string, a null or `unknown` instead of being left out.

#### Scenario: A PID collision does not attribute the runtime's own binary to the application

- **GIVEN** the sidecar topology where the runtime's own process ID equals the process ID the bus daemon reports for the application
- **WHEN** the application node's attributes are read
- **THEN** its process-ID attribute SHALL report the application's own process ID, and its process-table attributes SHALL be absent and SHALL in particular not describe the automation binary or any unrelated local process
- **NOTE** Real provider only. Measured today: the target application reports the automation binary's own name as its process name, and with an unrelated process parked on the colliding ID, that process's name, command line and executable path. That the reported process ID equals the runtime's own is not a finding: it is the application's number, and it is not compared with ours.

#### Scenario: On an ordinary desktop the process attributes are unchanged

- **GIVEN** a single-namespace desktop session where the bus daemon resolves process IDs
- **WHEN** an application node's attributes are read
- **THEN** the process-ID attribute and the process-table attributes SHALL describe that application exactly as before
- **NOTE** Real provider only; this is the regression guard for the normal case, where the process ID the daemon reports and the one valid in the runtime's namespace are the same number.
