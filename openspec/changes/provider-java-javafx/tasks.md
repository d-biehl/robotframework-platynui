<!-- Forward design: §1 is the FX spike on top of the infrastructure delivered by
     java-agent-core + provider-java-swing; §§2–4 build the adapter.
     Depends on java-agent-core, provider-java-swing and add-javafx-test-app. -->

## 1. Spike (FX-specific unknowns against the FX fixture)

- [ ] 1.1 Scene-graph read on the FX Application Thread with the agent's deadline discipline, incl. the agent-loads-before-FX-starts case (toolkit-initialized detection)
- [ ] 1.2 Module access on FX 11+/17/21 via `Instrumentation.redefineModule` (glass window handles, virtualized cells) vs. plain reflection on FX 8; pin the per-version matrix
- [ ] 1.3 Physical-pixel coordinate conversion for FX on a HiDPI monitor (JDK 8 / 17 / 21)
- [ ] 1.4 Fold results into design.md; refine §§2–4

## 2. Agent: JavaFX adapter

- [ ] 2.1 Spine (`Stage`/`Scene`/`Node` + `PopupWindow`s) + accessibility enrichment (`queryAccessibleAttribute`/`AccessibleRole`); full attribute surface incl. `Node.getId()` and model bulk reads (`TableView`)
- [ ] 2.2 Virtualized-control subtrees (off-screen `TableView`/`ListView` cells) read from the model, per design 1
- [ ] 2.3 Actions/patterns parity with Swing: focus, text edit, point hit-test + highlight for the picker; weak-ref registry ids and physical-pixel coords reused
- [ ] 2.4 Window handle: glass internals first, PID+geometry fallback; `toolkits` gains `"javafx"` in the handshake file

## 3. Provider + claims

- [ ] 3.1 FX role/attribute normalization in the `provider-java` mapping layer (`AccessibleRole` → PlatynUI roles, `native:*` attributes)
- [ ] 3.2 Claims: the Java provider claims FX windows served by the agent backend — UIA skips them on Windows (boolean claims, as for JAB); first claim on Linux

## 4. Acceptance & verification

- [ ] 4.1 Windows: FX fixture catalog controls resolve through the agent; UIA-vs-agent single representation for the same Stage
- [ ] 4.2 Linux: the FX fixture is reachable at all (the flagship scenario) — tree, locators, and the blueprint's last-action observables work with no native accessibility present
- [ ] 4.3 A Swing-only JVM is untouched by the FX adapter; a no-agent FX app on Windows still resolves via UIA
- [ ] 4.4 `just check`/`test`/`build-native` + the relevant acceptance lanes green
