<!-- Extracted from provider-java-swing, whose task 1.3 could not verify its HiDPI
     cell (host at 96 DPI, no JDK 17). Section 1 is a DECISION AND A MEASUREMENT
     and it gates everything below it: until a scaled display can be produced,
     nothing here can be measured, and coverage that did not run must say so
     rather than pass. Same shape as java-provider-linux's task 1.1, deliberately. -->

## 1. Settle how a scaled display is obtained (gates sections 2–5)

- [ ] 1.1 **Decide first** (design 1): evaluate a second physical monitor, a Windows display-settings change, and a virtual display driver against three questions — does it exercise mixed-DPI honestly, can a human repeat it, can CI run it? Record the answer in design.md; do not build coverage against a mechanism that has not been shown to work
- [ ] 1.2 Write down the procedure for putting a machine into each verified state (scale, monitor layout, what needs a sign-out), so a later run can reproduce the one that found or cleared a defect
- [ ] 1.3 Decide whether a virtual display is *evidence* or only *regression coverage* — if it is not faithful enough to establish a baseline, say so in the result rather than treating it as equivalent to real hardware
- [ ] 1.4 Install JDK 17 (design 5: 8 and 21 bracket the coordinate question, 17 confirms — its absence must not block the matrix, but a disagreement between 17 and 21 outranks everything else here)

## 2. Coverage that fails honestly

- [ ] 2.1 A scaled-display harness that reports the **scale and monitor layout it ran against** as part of its result (spec: "A run states the scale it ran at") — a pass at 100 % must not be mistakable for coverage of 150 %
- [ ] 2.2 The same harness reports **not exercised** rather than passing when no scaled display is available (spec: "Coverage that cannot run says so"). This is the guard that keeps the gap from silently reopening
- [ ] 2.3 Tolerance helper per design 3: agreement is ≤1 physical pixel per edge plus documented platform rounding, expressed once so no test invents its own bar

## 3. Measure the coordinate contract (Rust live fixtures, design 2)

- [ ] 3.1 `provider-windows-uia`: reported `Bounds`/`ActivationPoint` against the platform's own geometry for the same element, at 100/125/150/200 %
- [ ] 3.2 JAB backend: the same comparison — **the first execution of its per-window DPI transform anywhere**, since it is identity at 100 %. Record what the derived transform actually is at each scale, not only whether the result agreed
- [ ] 3.3 In-JVM agent backend: the same comparison, on JDK 8 (DPI-unaware) and 21 (per-monitor user space), and on 17 once 1.4 is done
- [ ] 3.4 Cross-provider agreement (spec: "The same element seen through two providers agrees"): one Java window described by the agent, the bridge and the UIA shell must yield the same rectangle
- [ ] 3.5 Mixed-DPI: a window moved to a monitor with a different scale factor, re-read — the case the agent's per-monitor derivation claims exactness for, and the one a desktop-wide factor gets wrong
- [ ] 3.6 Scale change during a live runtime (spec: "Scale is not baked in at startup"): re-read after the scale changes and confirm the new numbers, including that JAB's cached per-window transform re-derives

## 4. Close the loop with real input (acceptance lane, design 2)

- [ ] 4.1 Click at a reported `ActivationPoint` on a scaled display and assert the **fixture** reports receiving it — agreeing rectangles can still both be wrong, so this is the property that actually matters
- [ ] 4.2 The neighbour check: no adjacent control receives the click, which is what distinguishes "correct" from "close enough to look right"
- [ ] 4.3 `element_at_point` round-trip: a point inside an element's reported bounds resolves back to that element or a descendant, never a sibling or the window
- [ ] 4.4 Suite tagging and lane wiring so the scaled suites are selected only where a scaled display exists, and are visibly skipped — not silently absent — elsewhere

## 5. Fix what the measurement found

- [ ] 5.1 Fix any provider that violates the contract, keeping the 100 % acceptance lane green (design risk: a scaling fix must not regress the path everyone actually runs)
- [ ] 5.2 If the JAB transform is wrong: diagnose precisely and **record** it (design 4) — the repair-or-narrow decision belongs to `jab-provider`, not here
- [ ] 5.3 Update `dev-docs/platform-windows.md` and `dev-docs/architecture.md` with what is now measured rather than asserted, including any limitation found
- [ ] 5.4 Replace `provider-java-swing`'s recorded HiDPI gap with the result, so its archived verification item points at an answer

## 6. Verification

- [ ] 6.1 `just check`, `just test`, `just build-native` green
- [ ] 6.2 The unscaled acceptance lane green — unchanged, as the guard against scaling fixes regressing 100 %
- [ ] 6.3 The scaled matrix executed and recorded per 2.1: which scales, which monitors, which JDKs, which providers, and what each answered
- [ ] 6.4 Real-provider live fixtures (`cargo nextest run -p platynui-provider-java -p platynui-java-agent --run-ignored ignored-only`) green at 100 % and at the scales 1.1 settled on
