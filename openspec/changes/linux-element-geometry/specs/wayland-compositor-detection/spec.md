# Spec Delta

## MODIFIED Requirements

### Requirement: A substituted window geometry is visible in the log

Where a window's position on screen comes from the window manager — a real platform top-level window — and the window manager cannot answer for that window, because it cannot resolve the window or cannot report its bounds, the window's bounds SHALL still be answered with the geometry the application's toolkit reports for it: that rectangle, reported as a successful read, with no error to the caller. The substitution SHALL be recorded as a warning that names the window, says why the window manager could not answer (including its error, which for the PlatynUI compositor's control channel names the socket path), and states that the toolkit's own geometry was used instead. Apart from that warning, the result SHALL be unchanged.

The warning SHALL NOT flood the log. It SHALL be emitted at most once per top-level window for as long as the window manager keeps failing for that window, however often the window's bounds are read and however often the tree is enumerated again; a window the window manager answers for again, and that later fails once more, SHALL be reported again. Each affected window SHALL be reported on its own, so one window's warning does not hide another's.

This requirement and *A gated capability reports unavailable instead of success* answer different questions and do not conflict. A gated capability is an action — highlight, screenshot, a window operation — and reporting success for it would claim an effect that did not happen, so it reports unavailable. Reading a window's bounds is a read that has a best-effort answer: on X11 the toolkit's geometry is in real screen coordinates, while on Wayland it is relative to the window itself and therefore off by the window's position. Bounds therefore keep the best-effort value and say in the log that it is one; gated actions do not pretend to have acted. On Wayland the warning is what makes the substituted value diagnosable at all. The window manager's own call still fails by name as *An identified PlatynUI session is not silently downgraded* requires; this requirement governs what the consumer of that failure does with a window's bounds.

Nodes whose geometry does not come from the window manager are unaffected: a node inside a window SHALL be placed relative to its window, with the window's substituted bounds as the origin while the window manager cannot answer, and SHALL NOT be warned about separately — its window's warning covers it — and a transient popup SHALL use the geometry source specified for popups.

#### Scenario: A top-level window whose window manager is unusable keeps its fallback bounds and is reported

- **GIVEN** a session identified as PlatynUI whose control channel is unusable, and an application with a top-level window positioned away from the screen origin
- **WHEN** that window's bounds are read
- **THEN** the read SHALL succeed with the rectangle the application's toolkit reports for the window
- **AND** the log SHALL contain a warning naming that window, the window manager's failure including the control-socket path, and that the toolkit's geometry was used instead
- **NOTE** Real provider only. On Wayland that rectangle is relative to the window — measured as `{0,0,600,500}` while the compositor's own geometry for the window was `{10,40,600,500}`, with a click by locator that reported success and landed at the wrong place — which is why the warning, not the value, is what makes this case diagnosable.

#### Scenario: Repeated reads of a failing window warn once

- **GIVEN** a top-level window for which the window manager keeps failing
- **WHEN** its bounds are read many times, including across repeated enumerations of the tree
- **THEN** the log SHALL contain exactly one such warning for that window, and every read SHALL return the same fallback rectangle
- **AND** a second top-level window failing in the same way SHALL get its own single warning
- **AND** if the window manager answers for the first window again and later fails for it once more, that later failure SHALL be warned about again

#### Scenario: A usable window manager's bounds come without a warning

- **GIVEN** a session whose compositor is identified and whose control channel works
- **WHEN** a top-level window's bounds are read
- **THEN** they SHALL be the window manager's geometry for that window, and no substitution warning SHALL be logged
- **NOTE** Applies to both Linux backends that share this provider, X11 and the PlatynUI compositor.

#### Scenario: Geometry that never came from the window manager is unchanged

- **GIVEN** a control inside a window, and a transient popup surfaced in the tree
- **WHEN** their bounds are read while the window manager is unusable
- **THEN** the control SHALL be placed at its window-relative position from the window's substituted bounds, the popup SHALL keep the geometry its own source provides, and neither SHALL produce a substitution warning of its own
- **NOTE** Real provider only. The rule is about the window's position on screen, which only the window manager knows; the window-relative position and popup geometry are answers in their own right, not substitutes.
