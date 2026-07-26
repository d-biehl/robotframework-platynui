<!-- Forward design: §1 is the walking skeleton that confirms the decided approach
     against the Swing fixture (the JVM we have — nothing here is Swing-specific);
     §§2–5 build it out. Ends where the toolkit adapters begin: the agent is in the
     JVM and answers. No PlatynUI node is surfaced by this change. -->

## 1. Walking skeleton

- [x] 1.1 Gradle product project `java/agent` (self-bootstrapping toolchain pattern, `-target 8`), `just build-java-agent`; `just build-native` stays JDK-free (missing JAR ⇒ diagnostic, not build failure). Wire the JAR build as a **hard prerequisite of the lanes and crate tests that need it**, the way the fixture builds are wired (per `acceptance-lane-selection`), so a stale or missing JAR fails loudly instead of silently skipping agent coverage
- [x] 1.2 Agent skeleton: `premain` + `agentmain`, toolkit self-detection reporting the active **set**, loopback `ServerSocket` + NDJSON-RPC, **handshake file** (`agent-<pid>` with port/token/`toolkits` list/version in an owner-only per-user dir — POSIX `0700` on Unix, a **user-restricted ACL on Windows**, verified there), token-checked + **exact-version** handshake, reserved notification frame (design 1/1a)
- [x] 1.3 Injection both ways against the Swing fixture: `-javaagent` at launch **and** the native-Rust attach into a running JVM; observe the JEP 451 warning on a modern JDK and confirm `-XX:+EnableDynamicAgentLoading` via `JAVA_TOOL_OPTIONS` works without touching the launch command (design 5)
- [x] 1.4 Fold results into design.md; refine §§2–5

## 2. Native attach transport (Rust)

- [x] 2.1 New crate `crates/java-agent` (crate name `platynui-java-agent`, per the repo's prefix convention) speaking the JVM attach protocol directly — Unix: `.attach_pid<pid>` + `SIGQUIT` + `/tmp/.java_pid<pid>` socket; Windows: `OpenProcess`/`WriteProcessMemory`/`CreateRemoteThread` → `JVM_EnqueueOperation`, reply over a client-created named pipe (design 5)
- [x] 2.2 `loadAgent` of the PlatynUI agent JAR via that transport; distinct diagnostics for attach-failed vs. agent-init-refused (sandboxed-JNLP `SecurityManager`); focused review of the `unsafe` Windows leg. If it proves worse than expected, evaluate the narrow fallback (vendor jattach's Windows leg only, Apache-2.0)
- [x] 2.3 Wire-level RPC client in the same crate: handshake-file discovery, token + exact-version handshake, per-call deadlines, notification-frame handling — provider-neutral, usable without any toolkit adapter
- [x] 2.4 PID→JVM verification via the classifier's existing process signal (`jvm.dll` module list / `libjvm.so` in `/proc/<pid>/maps`); **no** machine-wide JVM enumeration — `jps`/`hsperfdata`-style listing is a non-goal (design 5)

## 3. Agent runtime (toolkit-neutral)

- [x] 3.1 Element registry: agent-assigned ids backed by **weak** references, plus the cheap liveness endpoint (id resolves to a live object still attached to a showing window) — design 2
- [x] 3.2 Multi-client RPC server, toolkit-thread-serialized, per-call deadline with abandon-on-timeout; UI-generation counter over the notification frame (design 7)
- [x] 3.3 Physical-pixel coordinate contract on the wire (design 3) — the conversion itself lands per toolkit with the adapters

## 4. Injection policy, packaging and delivery

- [x] 4.1 `providers.java.agent.enabled` (agent support usable at all) + `providers.java.agent.jar` (explicit override of discovery). The `auto_attach` flag belongs to the router and lands with `provider-java-swing` (design 6). The `providers.java.*` namespace comes from `unify-java-provider`, which may land before or after this change — keys are additive either way
- [x] 4.2 New package `platynui-provider-java` (`py3-none-any` wheel: JAR + `agent-path` CLI), entry point in the new `platynui.providers` group exposing `{ agent_jar, version }`; exact-pinned `[java]` extras on `robotframework-platynui` and `platynui-inspector`; release/wheel recipes updated (JAR mandatory there)
- [x] 4.3 Discovery transports: in-process PyO3/`importlib.metadata` lookup; Inspector/CLI binaries (`bindings = "bin"`, no wrapper process) resolve the co-located environment interpreter from their own exe path (fallbacks `pyvenv.cfg`/`VIRTUAL_ENV`) and run a one-shot query (JSON on stdout, cached); resolution order config → `PLATYNUI_JAVA_AGENT_JAR` env → entry point → actionable "install `robotframework-platynui[java]`" diagnostic
- [x] 4.4 `-javaagent` documented as the durable fallback, Attach as the primary path with its JEP-451 caveat and the `JAVA_TOOL_OPTIONS` remedy

## 5. Verification

- [x] 5.1 Multiple targets: handshake-file discovery across **two concurrent JVMs** (distinct ports/tokens, correct PID mapping, stale-file cleanup after a killed JVM)
- [x] 5.2 Concurrency: two client connections against one agent do not lock each other out; the deadline abandons a wedged toolkit-thread job and the client stays bounded
- [x] 5.3 Delivery: without `platynui-provider-java` nothing attaches and the diagnostic names the install; with it installed, discovery works over both transports; a version-mismatched agent is aborted with both versions named
- [x] 5.4 Quiescence: with the agent inactive, no handshake-directory scan, no attach, no agent JAR resolution; `just check`/`test`/`build-native` green
