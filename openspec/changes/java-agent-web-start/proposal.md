## Why

Java Web Start is one of the reasons the agent exists: the `java-agent` capability names "scripts, installers or Web Start" as why attaching into an already-running JVM is the primary path, and the JAB backend's fidelity gaps are exactly what an agent is meant to close. Measured against a real OpenWebStart 1.14 application on Java 8, the agent reaches **none** of it, for two independent reasons:

- The JNLP `SecurityManager` denies the agent its very first call (`getenv`), so it dies before publishing a handshake. The attach itself reports success, so the failure is silent — the provider sees a JVM that simply has no agent.
- Even with that fixed, the agent sees no windows. `Window.getWindows()` and `EventQueue.invokeLater` are scoped to the caller's AWT `AppContext`; IcedTea-Web gives every Web Start application its own, while the agent's threads live in the system one. Measured in the target: 2 AppContexts, the application's `JFrame` in the one the agent is not in.

Both defects are structural rather than incidental to OpenWebStart: the first hits any target with a restrictive security policy, the second any application that runs in its own `AppContext` (Web Start and applets by construction). Neither is reachable by configuration, so a locked-down test host has no workaround available today.

## What Changes

- The agent SHALL run with privileges independent of the target's security policy, by loading its classes through the bootstrap class loader (`Boot-Class-Path`), so a target-side policy can no longer decide whether PlatynUI works. Attaching an agent is already a fully privileged act, so this grants no reach the attach did not already have — and it is what IcedTea-Web itself does for its own runtime.
- A failed agent start SHALL stay inside the agent: no `Error` may escape `agentmain` into the target application, and the diagnostic channel SHALL survive a policy that denies it a property read — today the report of the failure throws its own failure and the operator learns only "Agent failed to start!".
- The Swing adapter SHALL see a JVM's **whole** UI: top-level windows are the union across all AWT `AppContext`s, and each element's calls are marshaled onto the event queue of the `AppContext` that owns it, not onto whichever queue the agent thread happens to reach. The per-call deadline and abandonment semantics stay as they are.
- The Rust side SHALL stop hiding this class of failure: an agent that was injected but never published a handshake is currently a `debug!` line, which is why the symptom presents as "nothing happens". It becomes a warning naming the target's own log as the place the cause was printed.
- Live coverage SHALL include a target that reproduces both conditions — a fixture launched under a restrictive policy and in a dedicated `AppContext` — so neither defect can return unnoticed. No OpenWebStart installation is required to run it. The policy half is bound to a JDK that still has a security manager (JEP 486 disabled it permanently in JDK 24), which is why it is pinned explicitly rather than left to the fixture's toolchain.

Not breaking for consumers: no protocol version bump, no configuration change, no node-shape change. One behavioural side effect on the **target**, on any JDK that ships a default CDS archive (12 and up, per JEP 341; measured on 21 and 24): appending to the bootstrap class path makes the JVM print `Sharing is only supported for boot loader classes...` and disables class-data sharing for the application's own classes. Making it conditional was considered and rejected in design — under `-javaagent` the agent runs before the security manager exists, so the condition cannot be evaluated when the decision has to be made.

## Capabilities

### New Capabilities

None. Both defects sit inside capabilities that already exist and already promise the behaviour that is missing.

### Modified Capabilities

- `java-agent`: the agent runtime gains two requirements it silently lacked — that it operates independently of the target's security policy (and fails visibly rather than silently when it cannot), and that "the toolkit thread" is resolved per `AppContext` rather than assumed to be one per JVM. The existing *Injection into a running JVM without launch changes* and *Bounded, multi-client agent runtime* requirements are the ones affected.
- `java-provider`: *Agent-backed Java UI tree* promises a running Swing application is served without launch changes; it gains the Web Start case as an explicit scenario, since that is the launch path the capability was justified by and the one it did not cover.
- `swing-test-app`: *Test-app CLI conventions* enumerates the fixture's CLI surface, which the two diagnostic launch modes extend. The modes are opt-in and the default launch is unchanged, but "a mode that cannot apply fails the launch" is a contract worth stating: a fixture that silently degraded to the default shape would keep every dependent test green while testing nothing.

## Impact

- **Java agent (`java/agent`)** — the bulk of the change:
  - `build.gradle.kts`: `Boot-Class-Path` manifest attribute.
  - `Agent.java`: version resource read via the class, not via `getClassLoader()`, which is `null` for a bootstrap-defined class.
  - `AgentLog.java`: the debug-property read must not take out the class initializer.
  - `AgentRuntime.java`: catch `Throwable`, not `IOException | RuntimeException`.
  - `SwingTree.java`, `SwingDispatcher.java`, `SwingAdapter.java`, `ElementRegistry.java`: AppContext-aware enumeration, dispatch and identity.
  - `ModuleAccess.java`: `sun.awt` access is already opened here for the native window handle; the AppContext work uses the same door on JDK 9+.
- **Rust (`crates/provider-java`)**: diagnostic level only (`agent/backend.rs`), no API or behaviour change. Needs a native rebuild for anyone testing the changed diagnostic.
- **Delivery (`packages/provider-java`)**: unchanged in content, but the JAR must be restaged — `just install-provider-java`, per the standing rebuild-is-not-delivery trap.
- **Tests**: agent JUnit tests, the live attach fixtures in `crates/java-agent/tests`, and the Windows acceptance lane (unchanged expectations — the Swing fixture must keep behaving exactly as today).
- **Platforms**: the agent-side work is platform-neutral and benefits Linux/macOS when `java-provider-linux` lands; the observable end-to-end path today is Windows, where `provider-java` is registered.
- **Docs**: `dev-docs/java-toolkits.md` — the injection-paths section states what a sandboxed JNLP target does to an agent, and now has measured answers plus the AppContext constraint, which is a fact about every Java toolkit adapter, not just Swing.
