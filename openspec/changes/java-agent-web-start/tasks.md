# Tasks

> Section 2's agent-side work (2.4–2.8) is implemented and was proven against a
> real OpenWebStart 1.14 / Java 8 application, but it is **not covered**: its tests
> (2.1–2.3, 2.9, 2.10) do not exist yet, and the failure mode of every one of these
> fixes is silence. Nothing here is finished until they do.

## 1. Fixture: reproduce both conditions without OpenWebStart

- [ ] 1.1 Add an opt-in launch mode to `apps/test-app-swing` that builds its UI inside a second AWT `AppContext` (new thread group + `SunToolkit.createNewAppContext()`), leaving the default mode byte-for-byte as today so the Windows acceptance lane is untouched
- [ ] 1.2 Add a policy file to the test fixtures granting the fixture's own code base `AllPermission` and nothing to anything else — the signed-`<all-permissions/>`-JNLP shape — and a launch mode that starts the fixture under `-Djava.security.manager` with it. **Pin the JDK that mode runs on and say why at the launch site**: JEP 486 permanently disabled the security manager in JDK 24, where `-Djava.security.manager` with any value but `disallow` makes the JVM refuse to start — so this mode is runnable on JDK ≤ 23 only, and a toolchain bump must fail with that sentence rather than with a JVM that will not come up. The grant must actually match the fixture's classes wherever the checkout lives, on Windows and Linux alike — a `codeBase` that matches nothing loads without error and silently leaves the fixture sandboxed too, which is a different and harsher shape than the one under test. Assert the fixture really holds the permissions, rather than trusting the policy to have applied
- [ ] 1.3 Extend the live-fixture harness in `crates/java-agent/tests/live_fixture.rs` so a test can request either mode, singly and combined
- [ ] 1.4 Update the `swing-test-app` capability's spec if the fixture's contract changes beyond an added mode

## 2. Agent operates independently of the target's policy

- [ ] 2.1 Live test: attaching into a fixture started under the restrictive policy publishes a handshake, accepts a connection and answers `ui/windows` (spec: *A sandboxed application still gets a working agent*)
- [ ] 2.2 Live test: a start-up failure — including one raised while reporting an earlier failure — leaves the target running and propagates no `Throwable` out of `agentmain` (spec: *A failed start never reaches the application*)
- [ ] 2.3 JUnit test: `AgentLog` initialises when the debug property read is denied. The other half — `Agent.version()` under a bootstrap-defining loader — is **not** a JUnit test and must not be written as one: a plain test cannot get its classes bootstrap-defined without `Instrumentation`, so it would exercise the app-loader path and pass for the wrong reason. Simulating it with `-Xbootclasspath/a:` alone is worse than useless: the resource lookup for a bootstrap-defined class goes through the *system* class path, so that test reports `"unknown"` and looks like a bug in the code it is testing. It belongs to 2.1's live attach, where the shipping artifact is loaded the way it really is
- [x] 2.4 Add `Boot-Class-Path: platynui-agent.jar` to the agent manifest in `java/agent/build.gradle.kts`, with the rationale from design decision 1
- [x] 2.5 Read the version resource through the class rather than through `getClassLoader()`, which is `null` for a bootstrap-defined class — recording at the call site that this resolves through the system class path, so the JAR being on both search paths is load-bearing rather than incidental
- [x] 2.6 Make `AgentLog`'s debug-property read incapable of taking out the class initializer
- [x] 2.7 Keep every failure of a start inside the agent. `AgentRuntime.start` catches `Throwable` around the **construction** as well — the class initializers of `AgentPaths` and `RpcServer` are where an `ExceptionInInitializerError` comes from, and leaving construction outside the guard reproduces the original defect one line higher. `Agent.start` carries a last-resort catch on top, because the spec's "including one raised while reporting an earlier failure" cannot be satisfied by a single catch whose own body does the reporting
- [x] 2.8 Make agent idempotence key on "an agent is running here" instead of "an attempt was made", so a failed start leaves the JVM open to the client's remaining attach attempts (spec: *A failed start does not disable the JVM for later attempts*). This is a **deletion**: `AgentRuntime.start` already guards on a live `instance`, so `Agent.started` was a second key with the wrong semantics, not a missing one
- [ ] 2.9 Pin the silent coupling with a test: the built JAR's `Boot-Class-Path` must name the file the build actually produces, and the artifact staged into `packages/provider-java` must carry that same name — a mismatch degrades to system-loader loading without any error
- [ ] 2.10 Verify on JDK 8 and on a current JDK that the bootstrap-loaded agent still resolves `javax.swing`, reads a full window tree, and still obtains the native window handle through `ModuleAccess`

## 3. Agent sees every toolkit world in the JVM

- [ ] 3.1 Live test: with the fixture's UI in its own `AppContext`, `ui/windows` returns that window and its tree is readable to full depth (spec: *An element outside the agent's own toolkit world is served*)
- [ ] 3.2 Live test: invisible owner windows and any second-context furniture stay out of the window list (spec: *The launcher's own windows are not part of the application*)
- [ ] 3.3 Live test: with two toolkit worlds and one event queue wedged, calls into the other keep being answered while the wedged one fails at its deadline (spec: *One wedged toolkit world does not disable the others*)
- [ ] 3.4 Add a reflective `sun.awt.AppContext` accessor to the agent (contexts, per-context window list, per-context event queue), degrading to the current single-context behaviour with a diagnostic when the internals are unavailable
- [ ] 3.5 Rewrite `SwingTree.windows()` to take the union across contexts, keeping the existing showing/active filtering and ordering
- [ ] 3.6 Make `SwingDispatcher` post to the event queue of the `AppContext` that owns the element, leaving `ToolkitDispatcher.Calls`' deadline and abandonment semantics unchanged
- [ ] 3.7 Record each element's owning `AppContext` in `ElementRegistry` at registration time — never derived per call from the agent's own thread — and keep liveness answerable when that context is gone
- [ ] 3.8 Route hit-testing through the same union: `SwingTree.chainAt` iterates `windows()`, so `ui/at_point` must find a window in another context and return its chain — cover it, since `element_at_point` is its own promised behaviour
- [ ] 3.9 Measure whether `SwingAdapter.watchStructuralChanges`' global `AWTEventListener` sees events dispatched on another context's event queue; register per context if it does not, and record the answer either way — a hint that never fires leaves clients with a stale tree
- [ ] 3.10 Confirm `ModuleAccess.openDesktopInternals` already opens what the accessors need on JDK 9+, and extend it only if it does not

## 4. The failure is visible on the client side

- [ ] 4.1 Rust test: an injected-but-silent agent produces the warning, not a debug line
- [ ] 4.2 Raise the "did not publish a handshake in time" diagnostic in `crates/provider-java/src/agent/backend.rs` to `warn!` and name the target's own log as where the cause was printed

## 5. Delivery and documentation

- [ ] 5.1 Rebuild and restage the agent JAR into `packages/provider-java` (`just install-provider-java`) — the installed package otherwise keeps serving the previous JAR under the same dev version
- [ ] 5.2 Record in `dev-docs/java-toolkits.md`: what a JNLP `SecurityManager` does to an agent and how the agent is now immune, the `AppContext` constraint as a fact about every Java toolkit adapter, and the JDK 9+ class-data-sharing side effect
- [ ] 5.3 Update `java/agent/README.md`, whose version-resource note already says "which class loader defines the agent package depends on the injection path" — that is now a design decision with consequences, not an incidental remark
- [ ] 5.4 Write the operator-facing side into `dev-docs/java-toolkits.md`'s "Getting the agent": what has to be true for a Web Start application to be served, given that the answer is meant to be "nothing beyond installing the extra" and is not quite. At minimum: that a JNLP cannot carry `-javaagent:` or `-XX:+EnableDynamicAgentLoading` (whitelist), so `JAVA_TOOL_OPTIONS` is the only channel for both; that attach needs the same user, a non-elevated target and an x86-64 JVM, and that endpoint protection blocking `CreateRemoteThread` looks like an attach failure; and that a version mismatch is remedied only by restarting the application, because an agent cannot be unloaded
- [x] 5.5 Carry the bootstrap-loader constraint (no application-class-path types from agent classes) into the `provider-java-javafx` and `provider-java-swt` designs — recorded there as JavaFX decision 3 / SWT decision 6, with a matching risk entry in each; nothing further is owed by this change

## 6. Verification

- [ ] 6.1 `just test-java-agent` and `just build-java-agent`
- [ ] 6.2 `just test-java-agent-live` — the new policy and `AppContext` scenarios are real-target-only and live here
- [ ] 6.3 `just check` and `just test` for the Rust-side diagnostic change
- [x] 6.4 Run the Windows acceptance lane and confirm it is unchanged at 115/115 — the Swing fixture's default mode must behave exactly as before. Done for the section 2 work with the rebuilt JAR actually staged (`just install-provider-java` first, or the lane exercises the previous artifact under the same dev version); repeat it for section 3, which is where the fixture gains launch modes
- [ ] 6.5 End-to-end confirmation against a real OpenWebStart application via `scripts/webstart-repro/run.ps1`: handshake published, window list non-empty, and — through the provider — the window served with the agent backend's `@Technology` and claimed once. This is what keeps the automated fixture honest: it is a *model* of OpenWebStart, and a model nobody compares against reality drifts green
