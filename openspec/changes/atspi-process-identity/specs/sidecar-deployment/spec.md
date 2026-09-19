# Spec Delta

## Purpose

PlatynUI running in a different PID namespace than the application it automates — the Kubernetes sidecar shape, where the application container holds the display server, the accessibility bus daemon and the application, and the runtime runs beside it. This capability defines the supported topology, what "our own process" means when process IDs from the two sides are not comparable, which behaviour degrades in which way when it cannot be established, and which process ID an application reports when it has one on each side.

## ADDED Requirements

### Requirement: PlatynUI automates an application in a foreign PID namespace

PlatynUI SHALL automate an application that runs in a different PID namespace, without requiring a shared PID namespace (`shareProcessNamespace` in Kubernetes terms). The supported topology is: display server, accessibility bus daemon and the application together in one namespace; the PlatynUI runtime in a sibling namespace; the sockets of both shared between them.

The deployment SHALL be expected to satisfy these prerequisites, and PlatynUI SHALL NOT be expected to work around their absence:

- Every endpoint the runtime uses is addressed by an **absolute** socket path that resolves identically on both sides — the accessibility bus address, the display-server socket and, on Wayland, the compositor control socket.
- The accessibility bus daemon runs in the application's namespace, so it sees the application but not the runtime.
- Both sides run under the **same uid**, because the bus daemon authenticates peers by their credentials.

Process IDs are the one thing the two sides cannot share. Every PID that crosses the boundary — reported by the bus daemon, by the display server or by the compositor — SHALL be treated as a value in the reporter's namespace, never as a value that can be compared with the runtime's own PID or with any other PID valid only in the runtime's namespace. Because the topology places the display server, the bus daemon and the application in one namespace, PIDs that those parties report about the same application are values in that one namespace and MAY be compared with each other. This requirement covers what the accessibility provider reports; the window-system half of the same rule is specified by the capabilities that own it — `element-at-point` for the window manager's own-window skip, `compositor-client-identity` for what the PlatynUI compositor reports about a client it cannot see and for its own skip of the caller's windows, and `wayland-compositor-detection` for identifying the compositor itself.

#### Scenario: The application tree is readable from a sibling container

- **GIVEN** an application, a display server and an accessibility bus daemon in one PID namespace, and the PlatynUI runtime in a sibling PID namespace with the bus socket shared and the same uid on both sides
- **WHEN** the tree is queried for the applications on that bus
- **THEN** every application registered on the bus SHALL appear in the tree, with its windows, roles, names and window-relative geometry
- **NOTE** Real provider only; verifiable with two PID namespaces and one shared bus socket, not with the mock.

#### Scenario: A PID collision does not empty the tree

- **GIVEN** the sidecar topology, and the runtime's own process ID happens to equal the process ID the bus daemon sees for the application
- **WHEN** the tree is queried
- **THEN** the application SHALL still appear in the tree, with the same result as when no PID coincides
- **NOTE** Real provider only. This is the reported defect: measured, the provider drops the application and returns an empty tree in exactly this situation, on dbus-daemon 1.14.10, dbus-daemon 1.16.2 and dbus-broker 35, on X11 and on the PlatynUI compositor alike.

#### Scenario: An application the daemon cannot resolve is not confused with another

- **GIVEN** the sidecar topology with two applications on the bus, for neither of which the daemon can report a usable process ID
- **WHEN** the tree is queried
- **THEN** both applications SHALL appear in the tree
- **NOTE** Real provider only. Measured on dbus-daemon ≤ 1.14 and dbus-broker up to version 37, which answer a process-ID query for such a peer with a successful `0`: an implementation that compares those zeros drops every application but the first, with no PID collision involved at all.

#### Scenario: An unreachable accessibility bus is reported, not mistaken for an empty bus

- **GIVEN** a runtime configured with an accessibility bus address that cannot be reached (wrong path, socket not shared, uid mismatch)
- **WHEN** the accessibility provider is asked for the applications on that bus
- **THEN** it SHALL fail with an error naming the configured bus address, and SHALL NOT answer with an empty list of applications
- **NOTE** Real provider only. How the runtime surfaces a failing provider is not specified here: its desktop enumeration logs the provider's error and continues with the remaining providers, so the error — and with it the bus address — reaches the log rather than the query result.

### Requirement: Process identity is decided once per bus connection from the daemon's view of that connection

The accessibility provider SHALL determine, once per connection to the accessibility bus, how it can recognise its own process on that bus, by asking the bus daemon for the credentials of **its own** connection. The outcome SHALL be exactly one of four modes:

- **Process-fd identity** — when the daemon returns a process file descriptor for our own connection, that descriptor refers to a process on the kernel's process filesystem (`pidfs`), and the process it pins is our own. Another connection is then ours exactly when the daemon returns a process file descriptor for it that pins the same process. This holds across PID namespaces, because these identities are namespace-independent.
- **Translated-descriptor identity** — otherwise, when the daemon returns a process file descriptor for our own connection whose process, expressed as a process ID in *our own* namespace, is our own process. Another connection is then ours exactly when its descriptor's process ID in our namespace equals ours. A descriptor whose process has no process ID in our namespace — the peer is invisible to us — is never ours. This mode covers kernels without `pidfs` on which the daemon still returns descriptors.
- **Process-id identity** — otherwise, when the daemon reports a process ID for our own connection that equals our own process ID. Daemon-reported process IDs are then values in our own numbering space, and another connection is ours exactly when its reported process ID equals ours.
- **No identity** — otherwise. The provider SHALL then perform no own-process check at all.

The check that the descriptor refers to a process on `pidfs` SHALL NOT be omitted from the process-fd test, and SHALL NOT be treated as implied by the descriptor pinning our own process: on kernels without `pidfs` every such descriptor shares one identity, so that test alone passes for every peer and would make the provider claim every application as its own. On such kernels the decision SHALL fall through to translated-descriptor identity, whose comparison the kernel performs per process rather than by shared identity.

The provider SHALL record the chosen mode and the inputs it was chosen from, once per connection, so that a mode lower than expected is diagnosable from a log. When the outcome is *no identity*, the record SHALL be a warning stating that own-process exclusion is inactive.

#### Scenario: A modern daemon on a modern kernel gives process-fd identity

- **GIVEN** credentials for our own connection that contain a process file descriptor, the descriptor lives on `pidfs`, and it pins our own process
- **WHEN** the mode is decided
- **THEN** the mode SHALL be process-fd identity
- **NOTE** Decidable from injected inputs without a bus. Measured on dbus-daemon 1.16.2 and dbus-broker 35/37 on kernels ≥ 6.9, both in and across PID namespaces.

#### Scenario: A process file descriptor that is not on pidfs does not give process-fd identity

- **GIVEN** credentials for our own connection that contain a process file descriptor which is **not** on `pidfs`, even though it appears to pin our own process
- **WHEN** the mode is decided
- **THEN** the mode SHALL NOT be process-fd identity, and the decision SHALL continue with the translated-descriptor test
- **NOTE** Decidable from injected inputs. Measured on Ubuntu 24.04's GA kernel 6.8 with dbus-broker 35 — the combination a stock Ubuntu 24.04 desktop runs — where every such descriptor is an anonymous inode with one shared inode number, so the "pins our own process" test succeeds for every peer.

#### Scenario: A descriptor without pidfs still identifies through the process ID in our namespace

- **GIVEN** credentials for our own connection that contain a process file descriptor which is not on `pidfs`, and whose process ID in our own namespace equals our own
- **WHEN** the mode is decided
- **THEN** the mode SHALL be translated-descriptor identity
- **NOTE** Decidable from injected inputs. Measured on Ubuntu 24.04's GA kernel 6.8 with dbus-broker 35: over 22 peer checks the process ID in the reader's namespace classified every peer correctly, with no false positive, where a shared-inode comparison classified every peer as our own.

#### Scenario: In translated-descriptor mode a peer we cannot see is never ours

- **GIVEN** translated-descriptor identity, and a peer whose process file descriptor has no process ID in our own namespace
- **WHEN** the peer is classified
- **THEN** the peer SHALL NOT be treated as our own process
- **NOTE** Decidable from injected inputs. This is the sidecar case: the kernel reports no process ID for a process in a namespace we cannot see, and an unresolved identity never matches.

#### Scenario: A process file descriptor pinning another process does not give process-fd identity

- **GIVEN** credentials for our own connection that contain a `pidfs` process file descriptor which pins a process other than our own
- **WHEN** the mode is decided
- **THEN** the mode SHALL NOT be process-fd identity, and the decision SHALL continue with the process-id test

#### Scenario: A daemon without process descriptors but with a matching process ID gives process-id identity

- **GIVEN** credentials for our own connection with no process file descriptor and a reported process ID equal to our own
- **WHEN** the mode is decided
- **THEN** the mode SHALL be process-id identity
- **NOTE** Decidable from injected inputs. Measured on dbus-daemon 1.12/1.14 and dbus-broker 29/33 within one namespace, which return no descriptor at all.

#### Scenario: A daemon that cannot see us gives no identity

- **GIVEN** credentials for our own connection from which no descriptor test succeeds — no process file descriptor at all, or one whose process has no process ID in our namespace — and a reported process ID that is `0`, absent, or different from our own
- **WHEN** the mode is decided
- **THEN** the mode SHALL be *no identity*, and a warning SHALL state that own-process exclusion is inactive
- **NOTE** Decidable from injected inputs. Measured for dbus-daemon 1.12/1.14 and dbus-broker 29/33 across PID namespaces, where the daemon reports `0` for a peer it cannot see.

#### Scenario: The mode is decided once and named in the log

- **GIVEN** a provider connected to the accessibility bus
- **WHEN** many tree queries run over that connection
- **THEN** the mode SHALL be decided once for that connection, and the log SHALL contain exactly one record for it naming the mode and the inputs it was decided from
- **NOTE** Real provider only for the "once" part; the provider holds more than one bus connection, and each decides and records for itself.

#### Scenario: A transient failure while deciding is retried, not frozen

- **GIVEN** the credentials query for our own connection fails transiently (timeout, I/O error)
- **WHEN** the next tree query runs
- **THEN** the mode SHALL be decided again rather than fixed to *no identity* by the failed attempt

### Requirement: An unresolved identity never matches

The provider SHALL treat "the daemon cannot tell me who this is" as *unknown*, and unknown SHALL never compare equal — not to our own identity, and not to another unknown. It SHALL exclude a connection as its own only on a **positive** match. A reported process ID of `0` SHALL never be accepted as an identity, whoever reports it.

#### Scenario: A peer the daemon cannot resolve is not our own

- **GIVEN** process-id identity, and a peer for which the daemon reports the process ID `0` or reports none
- **WHEN** that peer is classified
- **THEN** it SHALL NOT be classified as our own process
- **NOTE** Decidable from injected inputs. This is the case an implementation that resolves its *own* PID through the daemon gets wrong: both sides read `0` and every unresolvable application is discarded.

#### Scenario: A peer without a process descriptor is not our own

- **GIVEN** process-fd identity, and a peer for which the daemon returns no process file descriptor
- **WHEN** that peer is classified
- **THEN** it SHALL NOT be classified as our own process

#### Scenario: Our own connection is recognised across PID namespaces

- **GIVEN** process-fd identity in the sidecar topology, where the daemon reports nothing usable about our process IDs
- **WHEN** the provider's own accessibility connection is classified
- **THEN** it SHALL be classified as our own process
- **NOTE** Real provider only. Measured: across namespaces on a 1.16 daemon the process-ID comparison recognises neither its own connection nor anything else, so own-process exclusion is silently inactive; the descriptor comparison restores it.

#### Scenario: Nothing is filtered without identity

- **GIVEN** *no identity* mode
- **WHEN** the tree is enumerated and popup candidates are classified
- **THEN** no application and no popup SHALL be excluded as "our own", and every application on the bus SHALL appear in the tree

### Requirement: Own-process exclusion is applied consistently wherever the provider hides its own UI

The provider SHALL use one identity decision everywhere it excludes its own user interface on the basis of a **bus** peer's identity: enumerating the applications on the bus and deciding whether an event-driven popup candidate belongs to us. These SHALL NOT diverge — a positive match excludes in both, and an unresolved identity excludes in neither.

The point hit-test SHALL likewise not resolve the host's own UI, but it starts from a window the window system reported, not from a bus peer, and a window-system process identifier cannot be compared with a bus identity. Its exclusion SHALL therefore rest entirely on the window system's own ownership decision (`element-at-point`), and the provider SHALL NOT decide window ownership itself — neither by comparing a reported process identifier with its own, nor by any other re-derivation from a value whose numbering space it has not established. Whichever mechanism decides, the rule is the same: exclude on a positive match, never on an unresolved one.

#### Scenario: The host process's own application is excluded from its own tree

- **GIVEN** an identity mode other than *no identity*, and the host process (for example the Inspector) has its own accessible application registered on the same bus
- **WHEN** the tree is enumerated
- **THEN** the host's own application SHALL NOT appear in the tree
- **NOTE** Real provider only; the mock has no bus and no processes to compare.

#### Scenario: The point hit-test does not resolve the host's own UI

- **GIVEN** a window system that has established that its view of the runtime's own connection carries the identifier the runtime knows itself by, and a point over a window of the host process
- **WHEN** the element at that point is resolved
- **THEN** the provider SHALL NOT return an element of the host process
- **NOTE** Real provider only. The exclusion is the window system's: it skips the own-process window before the provider ever sees a hit. How the window system establishes ownership, and what it does when it cannot, is specified by `element-at-point`; this scenario only states that the provider does not undo it.

#### Scenario: The hit-test does not re-derive ownership from a foreign number

- **GIVEN** the sidecar topology, where neither the bus daemon nor the display server numbers processes the way the runtime does
- **AND** the window system therefore resolved a window whose reported process identifier happens to equal the runtime's own
- **WHEN** the element at that point is resolved
- **THEN** the provider SHALL NOT discard that window on the ground that its reported identifier equals the runtime's own process identifier
- **NOTE** Real provider only. Measured today as the opposite: under a forced collision the window is dropped a second time at the provider level, so `element-at-point` answers *No element* even once the window manager returns the window. The provider therefore performs no ownership comparison of its own on this path at all (design.md D10), which also covers the narrower case of a bus whose numbering *is* the runtime's while the display server's is not.

#### Scenario: Without identity, the host's own UI is no longer hidden

- **GIVEN** *no identity* mode, and the host process has its own accessible application on the same bus
- **WHEN** the tree is enumerated
- **THEN** the host's own application MAY appear in the tree, and this SHALL NOT cause any other application to be hidden
- **NOTE** Real provider only. This is the accepted loss of the fallback: exposing our own UI is recoverable by a query, mistaking somebody else's application for ours is not.

### Requirement: An application reports its own process ID, and process-table data only through a process ID valid in the runtime's namespace

An application node SHALL report as its process ID the number the application's own environment knows it by, and SHALL read process-table data only through a process ID valid in the runtime's own namespace. Across a PID-namespace boundary these are two different numbers for one application: the process ID the accessibility bus daemon reports for the application's connection, and — where the runtime can see the process at all — the one it has in the runtime's own namespace. In a shared namespace they are the same number. The provider SHALL NOT compare the two with each other, and SHALL NOT compare either with a process ID from another namespace; reporting a process ID as an attribute is not a comparison.

The process-ID attribute of an application node (`@ProcessId` on `app:Application`) is the application's identity. It SHALL report the process ID as the application's own environment knows it: the process ID the accessibility bus daemon reports for that application's connection. It SHALL be reported whether or not that number is valid in the runtime's own namespace. It SHALL be **absent** when the daemon cannot tell — it omits the process ID, reports `0`, answers that the process ID is unknown, or the lookup fails — and it SHALL NEVER be `0`.

Every attribute read from the local process table — process name, executable path, command line, user name, start time, architecture — SHALL be read only through a process ID valid in the runtime's own namespace, and SHALL be **absent** when the application has none there. The provider SHALL NOT read the local process table with a number valid only in another namespace, and SHALL NOT guess.

A locally valid process ID is a precondition for those attributes, not a guarantee of them: each process-table attribute SHALL be reported only when its value was actually read for that process, and SHALL be absent otherwise. In particular, an attribute the provider cannot determine SHALL NOT be answered with a substituted value — an empty string, a placeholder, or a value that describes the automation host instead of the application; a plausible wrong answer is worse than a missing one, because nothing distinguishes it from a real one. The presence of the process-ID attribute SHALL NOT imply the presence of the process-table attributes.

The identifier a consumer reads from an application node SHALL be the value of its process-ID attribute when that attribute is present, and otherwise what the node reports without a process ID — the toolkit's accessible-id, or nothing. It SHALL NEVER be `0`.

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

- **GIVEN** the sidecar topology, and an application whose process ID the bus daemon reports while the application has no process ID in the runtime's namespace
- **WHEN** that application node's attributes are read
- **THEN** the process-ID attribute SHALL be present, and every process-table attribute SHALL be absent
- **NOTE** Decidable from injected inputs; end to end only against a real provider across two PID namespaces. This is the normal picture of a sidecar deployment. Measured today: the process-table attributes are present and empty or wrong there, because they are read with the reported number.

#### Scenario: An attribute that cannot be determined is absent, not guessed

- **GIVEN** an application whose process the provider cannot read
- **WHEN** its process-table attributes are read, including the architecture attribute
- **THEN** every such attribute SHALL be absent, and none SHALL report a placeholder or a value taken from the automation host instead of the application
- **NOTE** Real provider only. Measured today: the architecture attribute falls back to the architecture the runtime was built for, so an application the runtime cannot even see is reported as `x64` — indistinguishable from a real answer, and wrong outright on a container of another architecture.

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

#### Scenario: An application's node identifier follows its process ID

- **GIVEN** an application for whose connection the bus daemon reports the process ID `N`, whether or not `N` is valid in the runtime's namespace
- **WHEN** its node identifier is read
- **THEN** the identifier SHALL be `N`, the same value as its process-ID attribute
- **NOTE** This is the node identifier a consumer reads (`element.id` in the Python surface), not the XPath attribute `@Id`, which is the toolkit's accessible-id — set when the toolkit provides one, empty when it does not. Decidable from injected inputs; on an ordinary desktop observable through the real provider.

#### Scenario: An application without a process ID has no PID-derived node identifier

- **GIVEN** two applications on the bus for which the bus daemon cannot tell the process ID, in any of the three ways listed above
- **WHEN** their node identifiers are read
- **THEN** neither identifier SHALL be `0` or any other value derived from a process ID; each SHALL be the toolkit's accessible-id when the toolkit provides one, and absent otherwise
- **NOTE** Decidable from injected inputs. Measured today on dbus-daemon 1.14 and dbus-broker: the node identifier reads `0` for every such application — one identifier for distinct applications.

#### Scenario: On an ordinary desktop the process attributes are unchanged

- **GIVEN** a single-namespace desktop session where the bus daemon resolves process IDs
- **WHEN** an application node's attributes and node identifier are read
- **THEN** the process-ID attribute, the process-table attributes and the node identifier SHALL describe that application exactly as before
- **NOTE** Real provider only; this is the regression guard for the normal case, where the process ID the daemon reports and the one valid in the runtime's namespace are the same number.

### Requirement: Only definitive identity answers are remembered

The provider SHALL cache an identity answer only when it is definitive: a resolved identity, or the daemon's explicit statement that it cannot resolve the connection. A transient failure — a timed-out or failed call — SHALL NOT be remembered and SHALL be retried on the next occasion. Shutting the provider down SHALL discard everything cached.

#### Scenario: A resolved identity is asked for once

- **GIVEN** an application that has been classified once on a connection
- **WHEN** the same application is classified again on that connection
- **THEN** no further query SHALL be sent to the bus daemon for it

#### Scenario: A definitive "cannot resolve" is remembered

- **GIVEN** an application for which the daemon has definitively answered that it cannot resolve the process
- **WHEN** the same application is classified again
- **THEN** the remembered answer SHALL be used, and the application SHALL still be treated as not our own

#### Scenario: A timed-out lookup is retried

- **GIVEN** an application whose identity lookup timed out
- **WHEN** the same application is classified again
- **THEN** the lookup SHALL be retried rather than the timeout being treated as an answer
- **NOTE** This is the defect of remembering `None`: one slow response at the wrong moment otherwise mislabels an application for the rest of the connection's life.

#### Scenario: Shutdown discards cached identities

- **GIVEN** a provider with cached identity answers
- **WHEN** the provider is shut down
- **THEN** the cached answers SHALL be discarded, so a later connection starts from a fresh decision

### Requirement: Correlating a native window to an application needs a process ID both sides can express

Where a native window and an accessible application are correlated by process ID, the provider SHALL correlate on the application's process ID as the bus daemon reports it — the value of its process-ID attribute — and compare it only with the process ID the window system reports for the window. The correlation runs in both directions: the point hit-test maps a window to its application, and the window manager maps an application node to its window and its popups through the node's process-ID attribute. Under this capability's topology both number the application in the application's own namespace, so the comparison stays within one namespace. Neither side SHALL be compared with the runtime's own process ID or with a process ID valid only in the runtime's namespace. When either side has no process ID, or only `0`, the provider SHALL report that nothing was resolved rather than correlate on an unusable value.

#### Scenario: A co-located application's window is correlated through its reported process ID

- **GIVEN** the sidecar topology, a window whose owner the display server reports as the process `N` in the application's namespace, and an application for whose connection the bus daemon reports `N`, where `N` is not valid in the runtime's namespace
- **WHEN** the element at a point inside that window is resolved
- **THEN** the provider SHALL correlate the window with that application and resolve the element of that application at the point
- **NOTE** Real provider only. Measured on main in the sidecar pod at non-colliding process IDs, where the target button resolved; under a collision it additionally needs the provider to stop re-deriving ownership (see *Own-process exclusion is applied consistently wherever the provider hides its own UI*).

#### Scenario: An application node's window and popups are found through its reported process ID

- **GIVEN** the sidecar topology, and an application for whose connection the bus daemon reports the process ID `N`, where `N` is not valid in the runtime's namespace
- **WHEN** the window manager is asked for that application node's window or for its popups
- **THEN** it SHALL be given `N` — the number the display server and the compositor use for the same application — and never a process ID valid only in the runtime's namespace
- **NOTE** Real provider only; the X11 window manager and the PlatynUI compositor backend read the process-ID attribute back from the application node, and the popup query filters the compositor's popups by the same number.

#### Scenario: No correlation is better than a wrong one

- **GIVEN** the sidecar topology, a window whose owner process the display server reports in the application's namespace, and an application whose process ID the bus daemon cannot tell
- **WHEN** the element at a point inside that window is resolved
- **THEN** the provider SHALL return no element, and SHALL NOT return an element of a different application
- **NOTE** Real provider only, and a known limitation for an application the daemon cannot see rather than a behaviour to be restored here: measured, the window reports its in-namespace process ID while the daemon reports `0` or nothing, so there is no number to correlate on.
