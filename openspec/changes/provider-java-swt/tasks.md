<!-- Forward design: §1 is the SWT spike on top of the infrastructure delivered by
     provider-java-swing; §§2–4 build the adapter. Depends on
     provider-java-swing and add-swt-test-app. -->

## 1. Spike (SWT-specific unknowns against the SWT fixture)

- [ ] 1.1 Widget-tree read on the SWT UI thread (`Display.syncExec`) with the agent's deadline discipline, incl. agent-loads-before-`Display`-exists
- [ ] 1.2 Subtree-scoped claims: confirm UIA enumeration can abstain at an agent-claimed shell and prune its native descendants without per-HWND bookkeeping
- [ ] 1.3 `setData` surfacing (incl. the SWTBot key) and `Control.handle` exactness against the fixture; physical-pixel conversion on win32 per-monitor DPI
- [ ] 1.4 Fold results into design.md; refine §§2–4

## 2. Agent: SWT adapter

- [ ] 2.1 Spine (`Display`/`Shell`/`Control`) + `Accessible` enrichment; full attribute surface incl. `getData()` keys as `native:*` attributes and `Table`/`Tree` model bulk reads
- [ ] 2.2 Virtual item subtrees (`TableItem`, `TreeItem`, `MenuItem`, `ToolItem`) with model-backed name/bounds/selection, per design 2
- [ ] 2.3 Actions/patterns parity with Swing: focus, text edit, point hit-test + highlight for the picker; weak-ref registry ids and physical-pixel coords reused
- [ ] 2.4 Exact native handles from `Control.handle` (win32; guarded per-platform reflection elsewhere); `toolkits` gains `"swt"` in the handshake file

## 3. Provider + claims

- [ ] 3.1 SWT role/attribute normalization in the `provider-java` mapping layer
- [ ] 3.2 `window_claims`: subtree-scoped abstention (a claimed window ⇒ its native descendants are skipped too); update the generic native-provider consumers; Swing/FX behavior unchanged

## 4. Acceptance & verification

- [ ] 4.1 SWT fixture catalog controls resolve through the agent, incl. a `setData`-keyed locator and virtual `Table`/`Tree` items
- [ ] 4.2 Single representation: UIA does not surface the control tree of an agent-claimed shell; an agent-less SWT app still resolves via UIA exactly as before
- [ ] 4.3 Robustness: deadline behavior on a blocked SWT thread; other providers unaffected
- [ ] 4.4 `just check`/`test`/`build-native` + the relevant acceptance lanes green
