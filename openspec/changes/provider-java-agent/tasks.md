<!-- Forward design: §1 is a spike that confirms the decided approach against the
     fixture (see design.md "Spike verification items"); §§2–6 build v1 for
     Swing/AWT only (JavaFX/SWT are later stages). -->

## 1. Spike (confirm the decided approach against the Swing fixture)

- [ ] 1.1 Agent walking skeleton: one JAR with `premain` + `agentmain`, EDT-marshaled tree read with an agent-side deadline, loopback `ServerSocket` + NDJSON-RPC, **handshake file** (`agent-<pid>` with port/token/version in a `0700` per-user dir), token-checked + **exact-version** handshake, reserved notification frame (design 1/1a/7)
- [ ] 1.2 Injection both ways against the fixture: `-javaagent` at launch **and** the native-Rust attach into a running JVM (attach driver also in the JAR for the JDK-on-host case); observe the JEP 451 warning on a modern JDK
- [ ] 1.3 Handshake-file discovery across **two concurrent target JVMs** (distinct ports/tokens, correct PID mapping, stale-file cleanup after a killed JVM)
- [ ] 1.4 Confirm the spike verification items in design.md: coordinate conversion on a HiDPI monitor (JDK 8/17/21), which window-handle internals work per JDK, the full element-model field list against the fixture
- [ ] 1.5 Fold spike results into design.md; refine §§2–6

## 2. Native attach transport (Rust)

- [ ] 2.1 New platform crate speaking the JVM attach protocol directly — Unix: `.attach_pid<pid>` + `SIGQUIT` + `/tmp/.java_pid<pid>` socket; Windows: `OpenProcess`/`WriteProcessMemory`/`CreateRemoteThread` → `JVM_EnqueueOperation`, reply over a client-created named pipe (design 5)
- [ ] 2.2 `loadAgent` of the PlatynUI agent JAR via that transport; distinct diagnostics for attach-failed vs. agent-init-refused (sandboxed-JNLP `SecurityManager`); focused review of the `unsafe` Windows leg
- [ ] 2.3 Native JVM discovery/listing (`hsperfdata` / attach protocol) — no `jps`/JDK required on the host

## 3. Agent (Swing/AWT, v1)

- [ ] 3.1 Tree read: instance-tree spine + per-node accessibility enrichment + virtual `AccessibleJTableCell` subtrees (design 2); **full** attribute/model surface incl. `Component.getName()` and `TableModel` bulk reads
- [ ] 3.2 Agent-assigned weak-ref registry for identity-stable element ids (design 2a); **physical-pixel** coordinate conversion in-JVM (design 2b)
- [ ] 3.3 Actions/patterns: focus (`requestFocus`), text edit (`setText`), point hit-test + highlight for the picker
- [ ] 3.4 Native window handle: in-JVM internals first, expose it for the provider's WindowManager delegation; provider-side PID+geometry fallback wiring (design 8)
- [ ] 3.5 Multi-client RPC server, EDT-serialized, per-call deadline with abandon-on-timeout; UI-generation counter over the notification frame (design 7)

## 4. Provider + routing (depends on `java-app-classifier`)

- [ ] 4.1 New crate `provider-java-agent`: NDJSON-RPC client (platform-neutral), connects via the handshake file (port + token)
- [ ] 4.2 Map agent elements → `UiNode` (normalized role/namespace, `native:*` attributes, patterns, identity-stable `RuntimeId`s); degraded-agent handling mirroring JAB's `DegradedTracker`
- [ ] 4.3 `window_claims`: **rank-based** ownership (agent > JAB > native-for-Java) replacing the boolean "claimed by other" check; update the existing consumers (design 3)
- [ ] 4.4 `java-app-classification` gains the "agent present?" signal (handshake file); routing is automatic, no attach/connect keyword

## 5. Injection policy + packaging

- [ ] 5.1 `providers.java-agent.enabled` + `providers.java-agent.auto_attach` (default off); off ⇒ agent only where launched with `-javaagent`
- [ ] 5.2 Agent compiled `-target 8`, shipped in the wheel; exact-version handshake refuses a stale agent with an actionable diagnostic (restart the JVM); `-javaagent` documented as the durable path, Attach as the JEP-451-sunset convenience

## 6. Acceptance & verification

- [ ] 6.1 Swing fixture: a `JTable` cell resolves with correct name/bounds/selection and a stable RuntimeId through the agent (the JAB gap closed)
- [ ] 6.2 Routing: agent outranks JAB for the same window (single representation); a no-agent JVM is still served by JAB
- [ ] 6.3 Concurrency: Inspector + test-run connections against one agent do not lock each other out
- [ ] 6.4 Robustness: an unresponsive agent stays bounded and does not block other providers; `just check`/`test`/`build-native` + the relevant acceptance lanes green
