# wayland-compositor-detection Specification

## Purpose

How the Wayland backend decides which compositor it is talking to, and what a capability — or a consumer of one — does when that answer does not support it. The decision gates window management, screenshots, highlighting, element-at-point and input-backend selection, so an identification that fails — or a gate that fails open — is not a missing feature but a wrong answer: a highlight that reports success without drawing, or window bounds that are silently off by the window's position. Identification must therefore rest on a channel that survives the deployments PlatynUI supports, including a runtime that runs in a different PID namespace than the application and the compositor; an action the backend cannot perform must be reported as unavailable to the caller that asked for it; and a window geometry that had to be substituted must be recognisable as such in the log.

## Requirements

### Requirement: The PlatynUI compositor identifies itself over its control socket

The PlatynUI Wayland compositor SHALL identify itself as a PlatynUI compositor in its control-socket status response, with a stable marker that does not depend on process names, paths or versions, alongside its version. The marker SHALL be part of the response every status request returns, so that one request answers both "is this a PlatynUI compositor" and "is its control channel usable". Adding it SHALL NOT change any existing field, so existing clients and the compositor control CLI keep working unchanged.

#### Scenario: A status request carries the identity marker

- **GIVEN** a running PlatynUI compositor with its control socket enabled
- **WHEN** a client sends the status request over that socket
- **THEN** the response SHALL report success, SHALL carry the PlatynUI identity marker, and SHALL still carry the fields it carries today (version, backend, uptime, socket name, XWayland state, window counts, outputs)

#### Scenario: Existing control-socket clients are unaffected

- **GIVEN** a client written against today's status response
- **WHEN** it queries a compositor that sends the identity marker
- **THEN** it SHALL keep working, reading the fields it knows and ignoring the marker
- **NOTE** Verifiable against the real compositor binary and the compositor control CLI, not the mock.

#### Scenario: The identity marker is not a side effect of any other command

- **GIVEN** a running PlatynUI compositor
- **WHEN** a client sends an unknown command
- **THEN** the response SHALL be an error as today, and SHALL NOT be mistakable for an identity answer

### Requirement: Compositor identification does not depend on process visibility

The Wayland backend SHALL identify the PlatynUI compositor without requiring the compositor's process to be visible to the runtime. Neither the peer credentials of the Wayland socket nor any `/proc` lookup derived from them SHALL be necessary for that identification, and a failure to read peer credentials SHALL NOT be interpreted as "this is not the PlatynUI compositor". The backend SHALL identify a session as PlatynUI when the session's control channel answers as a PlatynUI compositor, or when the session environment marks the session as a PlatynUI session. Compositors other than PlatynUI SHALL keep being classified from the peer process and the session environment as today, and a session with no PlatynUI evidence SHALL remain unidentified rather than be guessed.

#### Scenario: A runtime in the compositor's own namespace is unchanged

- **GIVEN** a PlatynUI compositor session where runtime and compositor share a PID namespace
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL identify the compositor as PlatynUI, and window management, screenshots, highlighting and element-at-point SHALL work exactly as before this change
- **NOTE** Regression anchor for the existing compositor acceptance lane; needs the real compositor, not the mock.

#### Scenario: A runtime in a sibling PID namespace identifies the compositor

- **GIVEN** a PlatynUI compositor and its application in one PID namespace, and the runtime in a sibling namespace that reaches the Wayland socket and the control socket by path, so the compositor's PID reads as 0 from the runtime
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL identify the compositor as PlatynUI
- **AND** a top-level window's reported bounds SHALL equal the compositor's own geometry for that window, including its position on screen, rather than a rectangle at the origin
- **NOTE** Verifiable only against a real compositor in a separate PID namespace (measured: `{10,40,600,500}` from the compositor versus `{0,0,600,500}` from an unidentified session).

#### Scenario: The session environment identifies the compositor when credentials are unreadable

- **GIVEN** a session whose environment marks it as a PlatynUI session and whose Wayland peer credentials cannot be read
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL identify the compositor as PlatynUI rather than as unrecognised

#### Scenario: A control channel that is not a PlatynUI compositor does not identify one

- **GIVEN** a configured control-socket path that no PlatynUI compositor serves — the path does not exist, nothing listens on it, or the peer answers without the identity marker
- **WHEN** the Wayland backend initializes in a session with no other PlatynUI evidence
- **THEN** it SHALL NOT identify the compositor as PlatynUI

#### Scenario: A foreign compositor is still recognised

- **GIVEN** a session running a compositor other than PlatynUI (for example Mutter, KWin, sway or Hyprland)
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL classify that compositor as it does today, SHALL NOT identify it as PlatynUI, and SHALL select the input backend that compositor supports
- **NOTE** Verifiable only against a real foreign compositor session, not the mock.

#### Scenario: A session with no PlatynUI evidence stays unidentified

- **GIVEN** a Wayland session with no PlatynUI control channel and no PlatynUI environment marker
- **WHEN** the Wayland backend initializes
- **THEN** the compositor SHALL be reported as unrecognised, and the capabilities that need a supported compositor SHALL report unavailable rather than guess

### Requirement: An identified PlatynUI session is not silently downgraded

When a session is identified as PlatynUI but its control channel is not usable, every capability that needs that channel SHALL fail with an error that names the control-socket path and the underlying cause. The backend SHALL NOT fall back to the behaviour it uses for an unrecognised compositor, and SHALL NOT report the session as unrecognised, because that fallback is what turns an unusable control channel into results that look plausible but are wrong.

#### Scenario: An identified session with an unusable control channel fails by name

- **GIVEN** a session identified as PlatynUI by its environment, whose control socket is missing or refuses connections
- **WHEN** a window-manager operation, a screenshot or a highlight is requested
- **THEN** each SHALL fail with an error naming the control-socket path and the underlying cause
- **AND** the error SHALL NOT claim that the compositor is unrecognised or unsupported

#### Scenario: A control channel that dies mid-session does not re-classify the compositor

- **GIVEN** a session identified as PlatynUI whose control channel worked at initialization and has since stopped answering
- **WHEN** the next gated operation is requested
- **THEN** it SHALL fail with an error naming the control-socket path
- **AND** the session SHALL remain identified as PlatynUI, so the failure is reported rather than replaced by foreign-compositor behaviour
- **NOTE** Verifiable only against a real compositor that is shut down during the session.

### Requirement: A gated capability reports unavailable instead of success

A capability the Wayland backend switches off because the identified compositor does not support it SHALL return an explicit unavailable error naming the capability and the compositor. It SHALL NOT return success, and SHALL NOT leave the caller to infer from a log message that nothing happened. An operation whose entire contract is that nothing is shown afterwards — clearing a highlight — MAY report success, because that postcondition holds even when the backend has nothing to clear.

#### Scenario: Highlighting under an unsupported compositor fails

- **GIVEN** a Wayland session whose compositor has no highlight backend
- **WHEN** a highlight of one or more rectangles is requested
- **THEN** the request SHALL fail with an unavailable error naming the highlight capability and the compositor
- **AND** the command-line client SHALL report that failure instead of printing that it highlighted regions
- **NOTE** Behaviour change: this is the case that reported success and exited 0 while logging that it did nothing.

#### Scenario: Clearing a highlight under an unsupported compositor succeeds

- **GIVEN** a Wayland session whose compositor has no highlight backend
- **WHEN** a highlight is cleared, including via an empty highlight request
- **THEN** the call SHALL report success, because no highlight is shown afterwards

#### Scenario: Screenshot and window management stay explicit

- **GIVEN** a Wayland session whose compositor is unrecognised
- **WHEN** a screenshot or any window-manager operation is requested
- **THEN** each SHALL fail with an unavailable error naming the capability and the compositor
- **NOTE** Regression anchor: this already holds today and must survive the change.

### Requirement: A substituted window geometry is visible in the log

Where a window's position on screen comes from the window manager — a real platform top-level window — and the window manager cannot answer for that window, because it cannot resolve the window or cannot report its bounds, the window's bounds SHALL still be answered with the geometry the application's toolkit reports for it, exactly as before: the same rectangle, reported as a successful read, with no error to the caller. The substitution SHALL be recorded as a warning that names the window, says why the window manager could not answer (including its error, which for the PlatynUI compositor's control channel names the socket path), and states that the toolkit's own geometry was used instead. Apart from that warning, the result SHALL be unchanged.

The warning SHALL NOT flood the log. It SHALL be emitted at most once per top-level window for as long as the window manager keeps failing for that window, however often the window's bounds are read and however often the tree is enumerated again; a window the window manager answers for again, and that later fails once more, SHALL be reported again. Each affected window SHALL be reported on its own, so one window's warning does not hide another's.

This requirement and *A gated capability reports unavailable instead of success* answer different questions and do not conflict. A gated capability is an action — highlight, screenshot, a window operation — and reporting success for it would claim an effect that did not happen, so it reports unavailable. Reading a window's bounds is a read that has a best-effort answer: on X11 the toolkit's geometry is in real screen coordinates, while on Wayland it is relative to the window itself and therefore off by the window's position. Bounds therefore keep the best-effort value and say in the log that it is one; gated actions do not pretend to have acted. On Wayland the warning is what makes the substituted value diagnosable at all. The window manager's own call still fails by name as *An identified PlatynUI session is not silently downgraded* requires; this requirement governs what the consumer of that failure does with a window's bounds.

Nodes whose geometry does not come from the window manager are unaffected: a node inside a window SHALL keep being placed relative to its ancestors as before and SHALL NOT be warned about separately — its window's warning covers it — and a transient popup SHALL keep using the geometry source specified for popups.

#### Scenario: A top-level window whose window manager is unusable keeps its fallback bounds and is reported

- **GIVEN** a session identified as PlatynUI whose control channel is unusable, and an application with a top-level window positioned away from the screen origin
- **WHEN** that window's bounds are read
- **THEN** the read SHALL succeed with the rectangle the application's toolkit reports for the window, exactly as it did before this change
- **AND** the log SHALL contain a warning naming that window, the window manager's failure including the control-socket path, and that the toolkit's geometry was used instead
- **NOTE** Real provider only. On Wayland that rectangle is relative to the window — measured as `{0,0,600,500}` while the compositor's own geometry for the window was `{10,40,600,500}`, with a click by locator that reported success and landed at the wrong place — which is why the warning, not the value, is what makes this case diagnosable.

#### Scenario: Repeated reads of a failing window warn once

- **GIVEN** a top-level window for which the window manager keeps failing
- **WHEN** its bounds are read many times, including across repeated enumerations of the tree
- **THEN** the log SHALL contain exactly one such warning for that window, and every read SHALL return the same fallback rectangle
- **AND** a second top-level window failing in the same way SHALL get its own single warning
- **AND** if the window manager answers for the first window again and later fails for it once more, that later failure SHALL be warned about again

#### Scenario: A usable window manager reports the same bounds as before, without a warning

- **GIVEN** a session whose compositor is identified and whose control channel works
- **WHEN** a top-level window's bounds are read
- **THEN** they SHALL be the window manager's geometry for that window, exactly as before this change, and no substitution warning SHALL be logged
- **NOTE** Regression anchor for both Linux backends that share this provider, X11 and the PlatynUI compositor.

#### Scenario: Geometry that never came from the window manager is unchanged

- **GIVEN** a control inside a window, and a transient popup surfaced in the tree
- **WHEN** their bounds are read while the window manager is unusable
- **THEN** each SHALL keep reporting the geometry its own source provides, and neither SHALL produce a substitution warning of its own
- **NOTE** Real provider only. The rule is about the window's position on screen, which only the window manager knows; ancestor-relative placement and popup geometry are answers in their own right, not substitutes.

### Requirement: The compositor identification and its basis are recorded

The Wayland backend SHALL record, once per session initialization, which compositor it identified, which mechanism decided it, and the inputs that mechanism saw — including the control-socket path and the outcome of the handshake, the session-environment value, and the outcome of the peer-credential read. A capability that is later refused because of that identification SHALL be traceable to this record. This exists so that a degraded identification is visible in a log instead of only in wrong coordinates.

#### Scenario: A successful handshake is recorded with its inputs

- **GIVEN** a session where the control-socket handshake identifies the compositor
- **WHEN** the Wayland backend initializes
- **THEN** it SHALL emit exactly one identification record naming the identified compositor, the handshake as the deciding mechanism, and the control-socket path used

#### Scenario: A degraded identification records why

- **GIVEN** a session where the control-socket handshake does not decide — no socket path, nothing listening, no identity marker, or a timeout
- **WHEN** the Wayland backend initializes
- **THEN** the identification record SHALL name the handshake outcome that prevented a decision and the mechanism that decided instead
