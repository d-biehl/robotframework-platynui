<!-- Forward design: §1 is a spike that confirms the decided approach against the
     fixture (see design.md "Spike verification items"); §§2–6 build the shared
     infrastructure + the Swing/AWT adapter (JavaFX/SWT adapters follow in the
     separate changes provider-java-javafx / provider-java-swt). -->

## 1. Spike (confirm the decided approach against the Swing fixture)

- [ ] 1.1 Agent walking skeleton: one JAR with `premain` + `agentmain`, EDT-marshaled tree read with an agent-side deadline, loopback `ServerSocket` + NDJSON-RPC, **handshake file** (`agent-<pid>` with port/token/`toolkits` list/version in a `0700` per-user dir), token-checked + **exact-version** handshake, reserved notification frame (design 1/1a/7)
- [ ] 1.2 Injection both ways against the fixture: `-javaagent` at launch **and** the native-Rust attach into a running JVM (attach driver also in the JAR for the JDK-on-host case); observe the JEP 451 warning on a modern JDK; if the Windows `unsafe` leg proves worse than expected, evaluate the narrow fallback (vendor jattach's Windows leg only, Apache-2.0 — design 5)
- [ ] 1.3 Handshake-file discovery across **two concurrent target JVMs** (distinct ports/tokens, correct PID mapping, stale-file cleanup after a killed JVM)
- [ ] 1.4 Confirm the spike verification items in design.md: coordinate conversion on a HiDPI monitor (JDK 8/17/21), which window-handle internals work per JDK, the full element-model field list against the fixture
- [ ] 1.5 Fold spike results into design.md; refine §§2–6

## 2. Native attach transport (Rust)

- [ ] 2.1 New platform crate speaking the JVM attach protocol directly — Unix: `.attach_pid<pid>` + `SIGQUIT` + `/tmp/.java_pid<pid>` socket; Windows: `OpenProcess`/`WriteProcessMemory`/`CreateRemoteThread` → `JVM_EnqueueOperation`, reply over a client-created named pipe (design 5)
- [ ] 2.2 `loadAgent` of the PlatynUI agent JAR via that transport; distinct diagnostics for attach-failed vs. agent-init-refused (sandboxed-JNLP `SecurityManager`); focused review of the `unsafe` Windows leg
- [ ] 2.3 PID→JVM verification via the classifier's existing process signal (`jvm.dll` module list / `libjvm.so` in `/proc/<pid>/maps`); **no** machine-wide JVM enumeration — `jps`/`hsperfdata`-style listing is a non-goal (only window-owning JVMs matter, design 5)

## 3. Agent (Swing/AWT, v1)

- [ ] 3.1 Tree read: instance-tree spine + per-node accessibility enrichment + virtual `AccessibleJTableCell` subtrees (design 2); **full** attribute/model surface incl. `Component.getName()` and `TableModel` bulk reads
- [ ] 3.2 Agent-assigned weak-ref registry for identity-stable element ids (design 2a); **physical-pixel** coordinate conversion in-JVM (design 2b)
- [ ] 3.3 Actions/patterns: focus (`requestFocus`), point hit-test + highlight for the picker; **no** text write — TextEditable stays a capability marker (`text-input-policy`), text is typed
- [ ] 3.4 Native window handle: in-JVM internals first, expose it for the provider's WindowManager delegation; provider-side PID+geometry fallback wiring (design 8)
- [ ] 3.5 Multi-client RPC server, EDT-serialized, per-call deadline with abandon-on-timeout; UI-generation counter over the notification frame (design 7)

## 4. Provider + routing (depends on `java-app-classifier`)

- [ ] 4.1 New crate `crates/provider-java` (name `platynui-provider-java`, matching the Python package): NDJSON-RPC client (platform-neutral), connects via the handshake file (port + token)
- [ ] 4.2 Map agent elements → `UiNode` (normalized role/namespace, `native:*` attributes, patterns, identity-stable `RuntimeId`s); degraded-agent handling mirroring JAB's `DegradedTracker`
- [ ] 4.3 Backend routing (design 3): the router prefers the agent backend when the agent-present signal holds, else JAB; mid-session agent appearance switches the serving backend on the next enumeration pass; `window_claims` untouched
- [ ] 4.4 `java-app-classification` gains the "agent present?" signal (handshake file); routing is automatic, no attach/connect keyword

## 5. Injection policy + packaging/delivery

- [ ] 5.1 `providers.java.agent.enabled` + `providers.java.agent.auto_attach` (default off); off ⇒ agent only where launched with `-javaagent`; `providers.java.agent.jar` as explicit override of discovery (namespace per `unify-java-provider`)
- [ ] 5.2 Gradle product project `java/agent` (self-bootstrapping toolchain pattern, `-target 8`); `just build-java-agent` recipe; justfile env wiring so `cargo run` binaries find the dev JAR; `just build-native` stays JDK-free (missing JAR ⇒ provider diagnostic, not build failure)
- [ ] 5.3 New package `platynui-provider-java` (`py3-none-any` wheel: JAR + `agent-path` CLI), entry point in the new `platynui.providers` group exposing `{ agent_jar, version }`; exact-pinned `[java]` extras on `robotframework-platynui` and `platynui-inspector`; release/wheel recipes updated (JAR mandatory there)
- [ ] 5.4 Discovery transports: in-process PyO3/`importlib.metadata` lookup in the native provider; Inspector/CLI binaries (`bindings = "bin"`, no wrapper process) resolve the co-located environment interpreter from their own exe path (fallbacks `pyvenv.cfg`/`VIRTUAL_ENV`) and run a one-shot entry-point query (JSON on stdout, cached); resolution order config → `PLATYNUI_JAVA_AGENT_JAR` env (override + `cargo run` dev leg) → entry point → actionable "install `robotframework-platynui[java]`" diagnostic; version mismatch aborts naming both versions; `-javaagent` documented as the durable path, Attach as the JEP-451-sunset convenience

## 6. Acceptance & verification

- [ ] 6.1 Swing fixture: a `JTable` cell resolves with correct name/bounds/selection and a stable RuntimeId through the agent (the JAB gap closed)
- [ ] 6.2 Routing: a JVM with the agent is served via the agent backend (single representation — one Java claim, one tree); a no-agent JVM is still served by the JAB backend
- [ ] 6.3 Concurrency: Inspector + test-run connections against one agent do not lock each other out
- [ ] 6.4 Robustness: an unresponsive agent stays bounded and does not block other providers; `just check`/`test`/`build-native` + the relevant acceptance lanes green
- [ ] 6.5 Delivery: without `platynui-provider-java` the provider reports the actionable install diagnostic and claims nothing (JAB fallback intact); with it installed, discovery works over both transports (test run in-process, Inspector via co-located-interpreter one-shot); a version-mismatched agent is aborted with both versions named
