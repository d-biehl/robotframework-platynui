# Spec Delta

## MODIFIED Requirements

### Requirement: Hit-test excludes the host process's own UI

Wherever the runtime can establish which UI is its own, hit-test SHALL NOT resolve an
element that belongs to the process hosting the runtime, consistent with top-down tree
enumeration (which excludes the own process under the same condition — see
`sidecar-deployment`). Where the window manager can see the stack (X11, the compositor),
an own-process window at the point SHALL be skipped so the window **behind** it is
resolved; otherwise resolving a point over own-process UI SHALL return nothing rather than
that UI. This prevents a point-based consumer (the Inspector live picker) from selecting
its own window or overlay. Where the runtime cannot establish ownership at all, it SHALL
NOT guess: the exclusion is dropped rather than applied to a number that means nothing,
and the own UI may then be resolved like any other.

Where hit-test goes through a window manager — on Linux, the X11 window manager and the
PlatynUI compositor — own UI SHALL be identified from the **window system's own view** of
which client owns a window, compared against the window system's view of the runtime's own
connection. A
process identifier that a window reports about itself SHALL NOT by itself be treated as
evidence of ownership: it is a number in the reporting client's process-identifier space,
which is the runtime's space only while both live in the same process namespace. Before
using such a reported identifier for the exclusion, the runtime SHALL establish — by asking
the window system about its **own** connection — that the window system's view of the
runtime carries the same identifier the runtime knows itself by.

When that check shows the two spaces do not agree, or the window system reports nothing
about the runtime's own connection, hit-test SHALL NOT exclude any window by comparing
reported process identifiers; a foreign window that merely reuses the runtime's number
SHALL be resolved like any other window. When the window system offers no way to ask about
the runtime's own connection at all, hit-test SHALL keep the previous behaviour (excluding
a window whose reported identifier equals the runtime's) and SHALL report once per
connection that the exclusion is running unverified, so the weaker guarantee is visible in
a normal run rather than silent.

On that path the window system's decision is the **only** own-UI exclusion in hit-test. Once
a window has been resolved, the provider SHALL NOT exclude it again by re-deriving ownership
from a process identifier it did not establish; a second comparison of the same
self-reported number can only undo a correct decision, never improve on it. Where the window
system can see the stack, a decision to exclude therefore means the window behind is
resolved and reported, not that the result is discarded.

On Windows, hit-test has no window-manager step: the accessibility API resolves the element at
the point directly and cannot be asked for the element behind it. There the provider SHALL
implement the exclusion itself, from the owning process the operating system reports for the
resolved element, and SHALL return nothing over the host process's own UI. That process is
reported by the operating system in the runtime's own numbering, so the comparison needs no
further check; the rules above about self-reported identifiers and about the provider not
re-deriving ownership govern the window-manager path only.

The deployment in which the identifier spaces differ — the runtime in a different process
namespace than the application, its display server and its accessibility bus — is the
`sidecar-deployment` capability introduced by the `atspi-process-identity` change; this
requirement only states what hit-test does there. That capability also owns the provider-side
code that stops re-deriving ownership; this requirement covers the window-manager level,
which on X11 is the X server's view of our own connection and under the PlatynUI compositor
is the identity the compositor captured for each connection.

#### Scenario: A point over the host process's own window is skipped

- **GIVEN** the window system's view of the runtime's own connection carries the same
  process identifier the runtime knows itself by
- **WHEN** hit-test is called with a point over a window belonging to the process hosting
  the runtime
- **THEN** it SHALL NOT return that process's own element — it resolves the window behind it
  (where the stack is known) or nothing
- **NOTE** Verifiable only against a real provider with an own-process window on screen, not
  the mock.

#### Scenario: On Windows the provider excludes the host process's own UI itself

- **GIVEN** a Windows session, where hit-test resolves the element at the point directly
  through the accessibility API
- **WHEN** hit-test is called with a point over a window belonging to the process hosting
  the runtime
- **THEN** the provider SHALL return nothing rather than the host process's own element
- **NOTE** Verifiable only against the real Windows UIA provider with an own-process window
  on screen, not the mock. This is today's behaviour on Windows and does not change.

#### Scenario: An application whose process identifier equals the runtime's is still resolved

- **GIVEN** the runtime runs in a different process namespace than the application, so the
  window system's view of the runtime's own connection does **not** carry the identifier the
  runtime knows itself by
- **AND** the application's window reports a process identifier numerically equal to the
  runtime's own
- **AND** the accessibility bus daemon can resolve that application, so the window can be
  correlated to it at all — where it cannot, `sidecar-deployment`'s requirement
  *Correlating a native window to an application needs a process ID both sides can express*
  applies instead and hit-test correctly resolves nothing
- **WHEN** hit-test is called with a point over that application's control
- **THEN** hit-test SHALL resolve that control rather than skipping the window, and the
  result SHALL be the same one it produces for the identical window when the numbers differ
- **NOTE** Verifiable only against a real display server with the runtime in a separate
  process namespace, not the mock. Measured today as the opposite: with the runtime forced
  onto the application's identifier, `element-at-point` returned *No element*, while control
  runs at neighbouring identifiers resolved the same button. Observable end to end once the
  provider stops re-deriving ownership from the window's reported identifier, which
  `sidecar-deployment` requires of it.

#### Scenario: The window system reports nothing about the runtime's own connection

- **GIVEN** the window system answers the question about the runtime's own connection with
  no identifier, or with a placeholder value that identifies no process
- **WHEN** hit-test is called with a point over any window, including one reporting the
  runtime's own identifier
- **THEN** hit-test SHALL NOT exclude a window by comparing reported process identifiers,
  and SHALL resolve the topmost window at the point as usual
- **NOTE** Verifiable only against a real display server that cannot see the runtime's
  process, not the mock.

#### Scenario: The window system cannot be asked, so the previous behaviour is kept and reported

- **GIVEN** the window system provides no facility to ask which process owns a connection
  (for example a display server without the required extension)
- **WHEN** hit-test is called with a point over a window whose reported process identifier
  equals the runtime's own
- **THEN** hit-test SHALL skip that window as before, and SHALL emit exactly one warning for
  that connection stating that own-UI exclusion is unverified on this display
- **NOTE** Verifiable against a real display server started without the extension; the
  single-warning behaviour is verifiable from the log of a run that performs several
  hit-tests.

#### Scenario: The ownership decision is taken once per connection and stated in the log

- **GIVEN** a runtime connected to a display server
- **WHEN** several hit-tests are performed over the lifetime of that connection
- **THEN** the runtime SHALL decide how own UI is identified once for that connection, log
  that decision and the values it rests on (the window system's view of the runtime's own
  connection and the identifier the runtime knows itself by) exactly once, and apply the
  same decision to every later hit-test on that connection
- **AND** a second runtime connecting afterwards — possibly to a different display — SHALL
  take its own decision rather than inherit this one
- **NOTE** Verifiable from the log of a real run; the per-connection (not per-process)
  scoping is what the multi-suite acceptance lanes exercise.
