## ADDED Requirements

### Requirement: Reported positions are physical desktop pixels at every display scale
Every position a provider reports — `control:Bounds`, `control:ActivationPoint`, and the geometry the window capability patterns operate on — SHALL be expressed in absolute desktop coordinates measured in **physical pixels**, and SHALL remain so when the display is scaled. A provider SHALL NOT report coordinates in a logical, DPI-independent or window-relative space, and SHALL NOT require the caller to apply a scale factor. Where a provider's underlying API answers in another space, the conversion is the **provider's** obligation and happens before the numbers leave it.

This holds for every provider that surfaces geometry, whichever channel it uses: the platform's native accessibility provider, an out-of-process bridge, and an in-process agent alike. Two providers describing the same element SHALL report the same rectangle within the tolerance a caller can act on.

#### Scenario: An element's reported rectangle matches where it really is
- **WHEN** an element is inspected on a display scaled to 125 %, 150 % or 200 %
- **THEN** its reported `Bounds` agrees with the rectangle the platform's own window/geometry API reports for the same element, to within a tolerance that does not exceed one physical pixel per edge plus any rounding the platform itself introduces

#### Scenario: The same element seen through two providers agrees
- **WHEN** a Java window is served by the in-JVM agent while the Access Bridge and the native accessibility provider can also describe it
- **THEN** the rectangles they report for the same element agree, so a locator's answer does not depend on which backend happened to serve it

#### Scenario: Scale is not baked in at startup
- **WHEN** the display scale changes while a runtime is alive, and an element is re-read afterwards
- **THEN** the newly reported coordinates reflect the new scale rather than the one in effect when the provider was created

### Requirement: A click at a reported activation point lands on its element
Pointer input driven from a reported position SHALL reach the element the position came from, at every supported display scale. This is the property callers actually depend on, and it SHALL be verified end to end rather than inferred from a coordinate comparison: a rectangle that is merely *self-consistent* still sends every click to the wrong place.

#### Scenario: Activating a control on a scaled display
- **WHEN** a control's `ActivationPoint` is read on a display scaled to 150 % and a click is synthesized at exactly that point
- **THEN** that control receives the click, and no neighbouring control does

#### Scenario: Hit-testing round-trips
- **WHEN** a point inside an element's reported bounds is handed to `element_at_point` on a scaled display
- **THEN** the element returned is that element, or one of its descendants — never an unrelated sibling or the containing window

### Requirement: Monitors with different scale factors are handled per monitor
On a desktop whose monitors carry different scale factors, coordinates SHALL be converted using the scale of the monitor the element is actually on, not a single desktop-wide factor. A provider SHALL NOT assume that the desktop's logical coordinate space is contiguous.

#### Scenario: An element on the secondary monitor
- **WHEN** a window is moved to a monitor whose scale factor differs from the primary's, and its elements are re-read
- **THEN** their reported coordinates place them on that monitor, and a click at a reported activation point lands on the intended element

### Requirement: The scaled-display verification is reproducible
The scale at which a verification ran SHALL be recorded with its result, and the procedure for putting a machine into that state SHALL be written down. A verification whose display configuration is unknown proves nothing about a later run, and an unreproducible one silently degrades into an untested claim — which is the state this change exists to end.

#### Scenario: A run states the scale it ran at
- **WHEN** the scaled-display coverage is executed
- **THEN** the display scale and monitor layout it ran against are part of the result, so a passing run at 100 % cannot be mistaken for coverage of 150 %

#### Scenario: Coverage that cannot run says so
- **WHEN** the coverage is executed on a machine that offers no scaled display
- **THEN** it reports that it was not exercised, and does not pass by default
