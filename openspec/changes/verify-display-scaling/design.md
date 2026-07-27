## Context

PlatynUI's central geometric promise is one sentence in `dev-docs/architecture.md`: *"Every position — `Bounds`, `ActivationPoint`, window geometry — is expressed in absolute desktop coordinates … Any DPI or scaling adjustment is handled on the provider side, before the numbers reach you."* Pointer input consumes exactly those numbers.

Nothing has ever checked that promise at a scale other than 100 %.

Three specifics make this more than a missing assertion:

- **The JAB backend's DPI transform is dead code on every machine we develop and test on.** It is derived per window from the JAB rectangle versus `GetWindowRect`, and `dev-docs/platform-windows.md` states it is *identity at 100 %*. It exists because JAB is system-DPI-aware while PlatynUI runs Per-Monitor-V2 — a mismatch that only produces different numbers when a scale factor is not 1. So the mechanism that corrects the mismatch has never run.
- **The in-JVM agent's conversion rests on a derivation, not a measurement.** `SwingGeometry` argues that the JDK derives a monitor's user-space bounds by dividing its device bounds by *that monitor's* scale, so the origin terms cancel and `device = user * scale` holds per monitor, mixed factors included. The argument is careful; it is still an argument.
- **Java 8 and Java 9+ reach the same answer by different routes.** Java 8 is DPI-unaware, so AWT coordinates already are device pixels and the transform is the identity; 9+ reports per-monitor user space. One code path normalises both, and only one of them has been exercised.

This change is extracted from `provider-java-swing`, which implemented the agent's conversion and could not verify its HiDPI cell: the development host reports 96 DPI and JDK 17 was absent. The scope widened on extraction because the blocker is a *display*, and one scaled display unblocks all three providers at once.

## Goals / Non-Goals

**Goals:**

- Settle how a scaled display is obtained for a real-provider run — the prerequisite everything else waits on.
- Measure, per provider, that reported geometry matches reality at 100 / 125 / 150 / 200 %, and on monitors with different scale factors.
- Close the loop with real input: a click at a reported activation point lands on its element.
- Fix what the measurement finds wrong.
- Leave coverage that is honest when it did not run, so this gap cannot silently reopen.

**Non-Goals:**

- **Changing the contract.** Physical desktop pixels stay the contract; this verifies it.
- **macOS.** Its provider is a stub (README platform table); there is nothing to measure.
- **A Linux bring-up.** The Wayland compositor already accepts `--scale`, which makes a second data point cheap — but Linux Java support is `java-provider-linux`'s subject, and X11/Wayland scaling semantics are their own topic.
- **Rewriting JAB's calibration.** Diagnosing it is in scope; replacing a heuristic that the agent backend already supersedes is not (see decision 4).
- **Fractional-scale rendering artefacts.** Whether a control is drawn crisply at 150 % is not a coordinate question.

## Decisions

1. **How a scaled display is obtained — OPEN, and it gates everything (decision by measurement).** Options, with what each actually proves:

    - **(a) A second physical monitor at a different scale.** The only option that exercises the *mixed-DPI* case honestly, because it is the real thing. Not available in CI, and its layout is not reproducible from a script.
    - **(b) Changing the primary display's scale in Windows settings.** Reproducible in the sense that a human can repeat it, but it changes the whole desktop for whoever is using the machine, some applications need a sign-out to pick it up, and it is not safe to do unattended.
    - **(c) A virtual display driver at a chosen DPI.** Scriptable, repeatable, and the only candidate that could ever run in CI — at the cost of installing a driver, and of proving that a virtual display is faithful enough to be evidence.
    - **(d) Per-application DPI overrides / DPI-unaware compatibility mode.** Rejected as the primary mechanism: it changes what the *application* believes rather than what the desktop is, which is a different question. It may still be useful to provoke the specific provider/target awareness mismatch JAB's transform exists for.

    Recommendation to validate, not to assume: **(a) for the authoritative local matrix, (c) if a virtual display proves faithful, for CI.** Until one of them exists this change cannot start — which is the same shape as `java-provider-linux`'s measurement gate, and deliberately so.

2. **Coverage is split by what each layer can actually assert.** The *coordinate* comparison belongs in the Rust live fixtures: both sides of it — the provider's `Bounds` and the platform's `GetWindowRect`/monitor geometry — are Rust APIs, and comparing them anywhere else adds a translation without adding evidence. The *click lands on the element* property belongs in the Robot acceptance lane, because it needs an application that reports what it received, which the Swing fixture already does with its last-action label. Neither layer can prove the other's half: agreeing rectangles can still both be wrong, and a landing click does not prove the rectangle.

3. **Agreement means within one physical pixel per edge, plus documented platform rounding.** Exact equality is the wrong bar — the JDK rounds, Win32 rounds, and the two need not round alike. A looser tolerance is worse than none: the defects this change hunts are *proportional* (a 25 % error on a 100-pixel element is 25 pixels), so any tolerance that a scale error could hide would make the test decorative.

4. **A JAB defect at scale is diagnosed here, fixed elsewhere (decided).** Its calibration is a heuristic by construction, and the agent backend exists to replace it for exactly this reason. If the measurement shows it wrong at 150 %, this change records *what* is wrong precisely enough to act on, and the decision whether to repair the heuristic or narrow the bridge's documented reach becomes a `jab-provider` matter. Grafting a rewrite onto a verification change would mix a measurement with a redesign.

5. **JDK 8 and 21 bracket the coordinate question; 17 is confirmation, not discovery (proposed).** For the *window handle*, 8 and 21 exercised genuinely different mechanisms. For *coordinates* the split is DPI-unaware (8) versus per-monitor user space (9+), and 17 sits inside the second family with 21. So 17 is worth running once it is installed, but its absence must not block the matrix — and if 17 disagrees with 21, that is a finding worth more than the rest of the change.

## Risks / Trade-offs

- [No scaled display materialises and the change stalls] → decision 1 is an explicit gate rather than a hopeful assumption, and the spec requires un-run coverage to report that it did not run instead of passing. A stalled change that says so is better than a green one that measured nothing.
- [Changing the development machine's scale disrupts whoever is using it] → prefer a second monitor or a virtual display over settings surgery; this is a real reason (b) is not the recommendation.
- [Fixing a provider's scaling regresses the 100 % path everyone actually runs] → the existing acceptance lane at 100 % is the guard, and it is already green; every fix here must keep it that way.
- [A virtual display is not faithful enough to be evidence] → then it is worth exactly one thing, catching regressions once the real matrix has established the baseline, and the proposal should say so rather than treat it as equivalent.
- [The measurement finds everything correct] → still worth it. The contract stops being a claim and becomes a checked property, and the JAB transform stops being code nobody has ever seen execute.

## Migration Plan

Additive. No behaviour changes unless a defect is found, in which case the change is a *correction* of numbers that were wrong at that scale — visible only to callers running scaled, who were already getting wrong answers. Rollback for any such fix is the ordinary one: revert the provider change; the verification coverage stays.

## Open Questions

- **The display mechanism** (decision 1) — the gate. Nothing downstream can be scheduled before it has an answer.
- **Can CI ever have a scaled display at all?** If the honest answer is no, this change delivers a documented manual matrix plus whatever regression coverage a virtual display supports, and that limitation belongs in the result rather than in a footnote.
- **Does the Wayland compositor's `--scale` give a cheap Linux data point** worth taking now, or does it only make sense once `java-provider-linux` has landed?
- **Is there a provider-independent way to provoke the awareness mismatch JAB's transform corrects**, so that mechanism can be exercised without waiting for hardware?
