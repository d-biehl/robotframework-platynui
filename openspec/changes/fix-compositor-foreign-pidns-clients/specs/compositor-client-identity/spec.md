# Spec Delta

## Purpose

Defines how the PlatynUI Wayland compositor determines which process a connected client belongs to, and what it does with that knowledge: when identity is established, that no control command may fail because a client is unidentifiable, how an unknown process is represented on the control socket, how an XWayland window's process id differs from a Wayland client's, and which window the compositor names at a point whose frontmost window belongs to the process that asked. Automation runs in which the compositor and the application do not share a PID namespace — the Kubernetes sidecar shape — depend on this, and so does every session that happens to hold one such client.

Identity here is always the compositor's **own** view of a connection's peer — never a number a client reports about itself. That is what makes it usable for a decision, and it is also what bounds it: the compositor can only decide about peers the operating system identifies to it.

## ADDED Requirements

### Requirement: A client's process identity is established when its connection is accepted

The compositor SHALL determine the process identity of every Wayland client from the peer credentials of the connection at the moment it accepts that connection, and SHALL keep the result for the lifetime of the connection. This applies to connections on the compositor's Wayland socket and to connections made through a security context. The compositor SHALL NOT determine a client's identity again while serving a request.

A peer the operating system does not identify to the compositor — a peer in a PID namespace the compositor cannot see reports a process id of `0` — and a credential read that fails SHALL both result in the identity *unknown*.

A client that cannot be identified SHALL be logged once, when its connection is accepted, at a level that is visible at the compositor's default log level, naming the reason. A client that is identified SHALL be logged once at a diagnostic level. Neither SHALL be logged again while serving requests, so a session in which no client can be identified is visible in an ordinary log without a per-request flood.

#### Scenario: A client in the compositor's own PID namespace is identified

- **GIVEN** a running compositor
- **WHEN** a client that shares the compositor's PID namespace connects and maps a window
- **THEN** every control-socket response about that window SHALL report that client's process id, and at the diagnostic log level the log SHALL contain exactly one line for that client stating that its process was identified

#### Scenario: A client the compositor cannot see is accepted as unidentified

- **GIVEN** a compositor started with its default log level, running in a PID namespace that does not contain the client's process
- **WHEN** that client connects and maps a window
- **THEN** the connection SHALL be accepted and served normally, and the log SHALL contain exactly one line for that client stating that its process could not be identified

  Verifiable only with two PID namespaces (the local namespace harness), not in the default test lane.

### Requirement: An unidentifiable client never breaks the compositor

No control-socket command SHALL fail, hang, or end the compositor session because a client's process cannot be identified. This covers every command that reports process ids — the window listing, including its minimized-window entries; a window looked up by index, stable id, application id or title; the window at a point; and the popup listing — whether the unidentified client's window is the one asked about or merely present in the session.

The compositor SHALL keep serving all other clients afterwards: a session that holds an unidentifiable client SHALL behave exactly like a session without one, apart from that client's reported process id.

#### Scenario: Window queries keep working while an unidentified client has a window

- **GIVEN** a compositor running in a PID namespace that does not contain the client's process, with that client's toplevel and popup mapped
- **WHEN** the window listing (with its minimized-window entries), the window at a point inside that window, that window looked up by application id, and the popup listing are requested in turn
- **THEN** every command SHALL return a successful response describing that window, and afterwards the compositor SHALL still answer a status request and SHALL shut down cleanly on request

  Verifiable only with two PID namespaces (the local namespace harness), not in the default test lane.

#### Scenario: An identified client is unaffected by an unidentified neighbour

- **GIVEN** a compositor with one window from an identified client and one from an unidentified client
- **WHEN** the window listing is requested
- **THEN** it SHALL contain both windows, and the identified client's entry SHALL carry its real process id

  Verifiable only with two PID namespaces (the local namespace harness), not in the default test lane.

### Requirement: An unknown process is reported as absent, never as zero

Wherever the control socket reports a window's process id — mapped windows, minimized windows, the window at a point, and popups — an unknown process SHALL be reported as JSON `null`. The compositor SHALL NOT report `0`, a negative number, or any placeholder process id, and SHALL NOT omit the field, so a consumer can tell "not identified" from a real process id without knowing how the compositor failed.

#### Scenario: An unidentified client's window carries a null process id

- **GIVEN** a compositor running in a PID namespace that does not contain the client's process, with that client's toplevel and popup mapped
- **WHEN** the window listing and the popup listing are requested
- **THEN** the window entry and the popup entry SHALL each contain a `pid` field whose value is `null`, and no entry in either response SHALL carry a `pid` of `0`

  Verifiable only with two PID namespaces (the local namespace harness), not in the default test lane.

#### Scenario: A response carrying no identified process is still well-formed

- **WHEN** any window-reporting response is produced for a client whose process is unknown
- **THEN** the response SHALL be valid JSON in which every other field of the entry carries its normal value, and the entry SHALL be distinguishable from an entry with a process id only by the `null`

### Requirement: An XWayland window reports the X11 client's declared process

For a window backed by an X11 client through XWayland, the reported process id SHALL be the process the X11 client declares for itself, and SHALL be `null` when it declares none. The compositor SHALL NOT substitute the peer credentials of XWayland's own connection, and SHALL NOT report an XWayland window's process id as unknown merely because XWayland's own connection carries no captured identity.

A declared process id is a value the X11 client chooses and the compositor cannot verify; it is reported as given — with one exception that the requirement *An unknown process is reported as absent, never as zero* imposes on every source: a declared `0` is not a process id, and SHALL be reported as `null` like a window that declares none.

#### Scenario: An X11 client that declares a process id of zero is reported as null

- **GIVEN** a compositor with XWayland running
- **WHEN** an X11 client that declares `_NET_WM_PID` as `0` maps a window and the window listing is requested
- **THEN** that window's entry SHALL carry a `pid` of `null`, because `0` is never reported whatever its source

  The decision over the declared value is verifiable in the default lane; end-to-end it needs a real `Xwayland` binary.

#### Scenario: An X11 client that declares its process is reported with it

- **GIVEN** a compositor with XWayland running
- **WHEN** an X11 client that declares its process id maps a window and the window listing is requested
- **THEN** that window's entry SHALL carry the declared process id

  Verifiable only with a real `Xwayland` binary, not in the default test lane.

#### Scenario: An X11 client that declares no process is reported as null

- **GIVEN** a compositor with XWayland running
- **WHEN** an X11 client that declares no process id maps a window and the window listing is requested
- **THEN** that window's entry SHALL carry a `pid` of `null`, not the process id of XWayland or of the compositor

  Verifiable only with a real `Xwayland` binary, not in the default test lane.

### Requirement: The window at a point is never the window of the process that asked

When asked which window is at a point, the compositor SHALL NOT answer with a window whose client is the process that asked. It SHALL answer with the window **behind** it — the frontmost window at that point that the asking process does not own — and with nothing when there is no such window. The answer SHALL otherwise be the one the compositor would have given: same fields, same values, same meaning of "frontmost".

The compositor SHALL decide this from its own view of both sides: the identity it captured when it accepted the window's client connection, and the identity of the peer of the **control connection the request arrived on**. The caller is therefore whoever asked, not the process that happens to own the frontmost window and not a process named in the request.

A window SHALL be excluded **only on a positive match** of two identities the compositor could establish. The compositor SHALL NOT exclude a window when the window's client could not be identified, when the caller could not be identified, or when the window's process is only a value its client declared about itself (an XWayland window's declared process id, which the compositor cannot verify and did not establish). Answering with an occluded window, or with nothing, on the strength of an identity that was never established is a worse answer than answering with the caller's own window.

This exclusion SHALL apply to the point lookup alone. Every listing — the window listing including its minimized entries, a window looked up by index, stable id, application id or title, and the popup listing — SHALL keep reporting the caller's own windows unchanged, because those answer "what exists", not "what is under this point".

#### Scenario: The caller's own window is skipped and the window behind is returned

- **GIVEN** a compositor that can identify both the caller and the clients concerned, with a window of another process at a point and a window of the asking process in front of it at the same point
- **WHEN** the asking process requests the window at that point
- **THEN** the response SHALL describe the other process's window, and SHALL NOT describe the asking process's own window

#### Scenario: The caller's own window is the only window at the point

- **GIVEN** a compositor that can identify the caller, with only the asking process's own window at a point
- **WHEN** the asking process requests the window at that point
- **THEN** the response SHALL be successful and SHALL report no window, rather than reporting the asking process's own window

#### Scenario: The caller's own window is still listed

- **GIVEN** the asking process has a mapped window and a mapped popup
- **WHEN** it requests the window listing, that window by application id, and the popup listing
- **THEN** each response SHALL contain that window, with the process id the compositor established for it

#### Scenario: The caller is the reference, not the owner of the frontmost window

- **GIVEN** two processes with a control connection each, and a point whose frontmost window belongs to the first of them
- **WHEN** both request the window at that point
- **THEN** the first SHALL be given the window behind it, and the second SHALL be given the frontmost window — the same point yields different answers because the callers differ

#### Scenario: An unidentifiable client's window is not excluded

- **GIVEN** a compositor that cannot identify the client owning the frontmost window at a point — the client lives in a PID namespace the compositor cannot see — and a caller whose own identity the compositor established
- **WHEN** the window at that point is requested
- **THEN** the frontmost window SHALL be reported, and no window SHALL be excluded

  The decision itself is verifiable in the default lane, because it is a decision over two identities; end-to-end it needs a client the compositor cannot see while the caller is one it can — a caller inside the compositor's PID namespace and a client outside it (the local namespace harness).

#### Scenario: An unidentifiable caller excludes nothing

- **GIVEN** a caller whose control connection the compositor cannot resolve to a process — a caller in a PID namespace the compositor cannot see, which is also the case for a caller in a sibling namespace
- **WHEN** it requests the window at a point over a window of its own
- **THEN** that window SHALL be reported rather than skipped, and the compositor SHALL NOT exclude any window on this request

  Verifiable only with two PID namespaces (the local namespace harness), not in the default test lane.

#### Scenario: An XWayland window is never excluded on its declared process id

- **GIVEN** a compositor with XWayland running and an X11 window that declares a process id equal to the caller's
- **WHEN** the caller requests the window at a point over that window
- **THEN** that window SHALL be reported, because a declared process id is not an identity the compositor established

  Verifiable only with a real `Xwayland` binary, not in the default test lane.

### Requirement: A restrictive security policy denies a client it cannot identify

When the compositor runs under a restrictive protocol policy, a client whose process cannot be identified SHALL NOT be granted privileged protocol access, and the decision SHALL be reached without ending the session. A client whose process is identified SHALL be judged as before, by its process name against the policy's list.

#### Scenario: An unidentifiable client is denied under a restrictive policy

- **GIVEN** a compositor running under a restrictive protocol policy
- **WHEN** it decides whether a client whose process could not be identified may use privileged protocols
- **THEN** the decision SHALL be "denied" and the compositor SHALL continue running

#### Scenario: A permissive policy allows an unidentifiable client

- **GIVEN** a compositor running without a restrictive protocol policy
- **WHEN** it decides whether a client whose process could not be identified may use privileged protocols
- **THEN** the decision SHALL be "allowed", exactly as for any other client
