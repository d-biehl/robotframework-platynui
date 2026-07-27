## Why

Every position PlatynUI reports — `Bounds`, `ActivationPoint`, window geometry — is absolute desktop coordinates in **physical pixels**, and pointer input aims at exactly those numbers. A systematic scale error is therefore the worst kind of defect this project can have: the tree still looks correct, every locator still matches, and every click lands somewhere else.

That contract has never been measured anywhere but at 100 %.

- **No test in the repository exercises a scale other than 100 %.** The only mention of DPI under `tests/` is a documentation string.
- **The JAB backend's per-window DPI transform is documented as "identity at 100 %"** (`dev-docs/platform-windows.md` §2a). It exists solely because JAB is system-DPI-aware while PlatynUI is Per-Monitor-V2 — so the entire calibration mechanism has never executed in anger on a development or CI machine.
- **The in-JVM agent was designed specifically to avoid that heuristic**, converting inside the JVM via `GraphicsConfiguration` "so the provider stays dumb" (`provider-java-swing` decision 3). That claim is equally unmeasured.

This change is extracted from `provider-java-swing`, whose task 1.3 could not close its HiDPI cell: the development host reports 96 DPI and no scaled display was available. Rather than archive that change with a verification hole, the hole becomes its own change — and widens, because the hardware prerequisite that unblocks the Java agent unblocks UIA and JAB at the same time, and the gap was never Java-specific.

## What Changes

- **A way to run the real-provider suites against a scaled display**, locally and in CI. This is the load-bearing unknown and it gates everything else; see design decision 1.
- **Measured agreement, per provider, between what the tree reports and where the element actually is**: each provider's `Bounds` and `ActivationPoint` against the platform's own geometry for the same element, at 100 / 125 / 150 / 200 %. Covers `provider-windows-uia`, the JAB backend and the in-JVM agent backend, because all three feed the same contract and only one of them (the agent) converts at the source.
- **The mixed-DPI multi-monitor case**, where the agent's conversion rests on a *derivation* — that the JDK's per-monitor user-space bounds make `device = user * scale` exact — which deserves a measurement rather than an argument.
- **The Java paths across JDK 8 / 17 / 21**, since Java 8 is DPI-unaware and 9+ is per-monitor user-space; the two are normalised by different code paths. JDK 17 is absent from the current development host and becomes a prerequisite.
- **Whatever fixes fall out.** The deliverable is a correct contract, not a report: if a provider is wrong at 150 %, this change fixes it.
- **Pointer input closes the loop**: a click driven through the reported `ActivationPoint` has to land on the element, which is the property users actually depend on and the one a bounds comparison alone does not prove.

## Capabilities

### New Capabilities

- `desktop-coordinate-contract`: what "absolute desktop coordinates in physical pixels" obliges every provider to deliver, and that the obligation holds under display scaling and across monitors with different scale factors — including that a synthesized click at a reported activation point lands on the element it came from.

### Modified Capabilities

None known at proposal time. This change *verifies* an existing contract rather than restating it; where a provider is found to violate it, the fix is an implementation change. Should a provider turn out to be unable to meet the contract — the plausible candidate is the JAB backend, whose calibration is a heuristic by construction — the resulting limitation is recorded against `jab-provider` in a follow-up rather than smuggled in here.

## Impact

- **Verification, not yet code**: `crates/provider-java/tests/live_fixture.rs`, `crates/provider-windows-uia`'s live coverage, and the `tests/acceptance` lanes gain scaled-display coverage; the lane wiring in `justfile` gains whatever the display mechanism needs.
- **Possible fixes**, depending on what the measurement says: `crates/platform-windows` (monitor scale reporting, `WindowManager` geometry), `crates/provider-windows-uia` (`BoundingRectangle` handling), `crates/provider-java-jab` (the self-calibrating transform — the most likely to be wrong, since it has never run), `crates/provider-java/src/agent` and `java/agent`'s `SwingGeometry` (the in-JVM conversion).
- **No native rebuild semantics change**, but the acceptance lane needs `just build-native` as usual; the Java paths additionally need `just install-provider-java`, and JDK 17 installed.
- **Platforms**: Windows first, because that is where all three providers and the fixture live, and where Per-Monitor-V2 is already active. Linux X11/Wayland is in scope only to the extent that the Wayland compositor already offers `--scale` for a scaled output — a cheap second data point, not a bring-up; macOS is out of scope (its provider is a stub, see the README's platform table).
- **Hardware/environment prerequisite**: a scaled display. Whether that is a second monitor, a display-settings change, a per-application DPI override or a virtual display is exactly what design decision 1 has to settle — and until it is settled, nothing here can be measured.
- No BREAKING changes intended. A fix that moves reported coordinates would be a *correction* of numbers that were wrong at that scale, and would be called out as such.
