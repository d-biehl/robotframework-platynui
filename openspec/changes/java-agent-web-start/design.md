# Design

## Context

The agent is measurably unable to serve a Java Web Start application. Both causes were reproduced against a real OpenWebStart 1.14 installation running the demo JNLP on Java 8 out of OWS's own JVM cache, and both were then reproduced without OpenWebStart, which is what makes them testable in CI.

**Cause 1 — the target's security policy.** IcedTea-Web installs a `JNLPSecurityManager`. The agent JAR is appended to the *system* class path by the attach, so its `ProtectionDomain` is an ordinary file code source with only the base policy's grants. Measured, from the OWS log of an attach into the running application:

```
Denying permission: ("java.lang.RuntimePermission" "getenv.PLATYNUI_AGENT_DIR")
Denying permission: ("java.util.PropertyPermission" "platynui.agent.debug" "read")
```

`AgentPaths.handshakeDirectory()` reads `getenv` as its first statement, so the agent dies before anything is published, while the attach reports success — the silent failure the operator sees as "the agent does not work". The second denial is worse than it looks: it comes from `AgentLog`'s static initializer, which runs *while reporting the first failure*, so the report throws an `ExceptionInInitializerError` — an `Error`, outside `AgentRuntime.start`'s `catch (IOException | RuntimeException)`. It escapes `agentmain` into the target's attach listener thread, and the operator is left with a bare `Agent failed to start!`.

**Cause 2 — AWT `AppContext`.** Measured inside the running OWS application by a diagnostic agent:

```
this thread's AppContext = sun.awt.AppContext[threadGroup=system]
Window.getWindows() from here: 1
AppContexts in this JVM: 2
  [threadGroup=system]                 -> 1 window:  SharedOwnerFrame showing=false
  [threadGroup=PlatynUI WebStart Demo] -> 4 windows: ... JFrame showing=true   <- the application
```

`Window.getWindows()` and `EventQueue.invokeLater` both resolve against `AppContext.getAppContext()` — the *calling thread's* context. The agent's threads (attach listener, RPC handlers) belong to the system context; ITW creates a dedicated context per application. So `SwingTree.windows()` finds only the invisible `SharedOwnerFrame`, which `isShowing()` filters out, and `ui/windows` returns `[]`. `SwingDispatcher` has the same defect one level down: it posts to the system context's event queue, which is not the thread that owns the application's components.

Constraints that shape the solution: the agent targets Java 8 source level and must keep working from 8 through current JDKs; it may not change the behaviour of the application it observes; and `sun.awt` is internal, so JDK 9+ access needs the module opening the agent already performs for the native window handle.

**For this target class, attach is not merely the primary path — through the application's own descriptor it is the only one.** IcedTea-Web validates a JNLP's `java-vm-args` against a fixed whitelist (`net.adoptopenjdk.icedteaweb.jvm.JvmUtils`: the `-XX:` GC and printing flags, the `--add-*` module options, and a restricted set of properties) and silently drops the rest — observed as `Ignoring java-vm-args due to illegal Property` for a plain `-D`. Neither `-javaagent:` nor `-XX:+EnableDynamicAgentLoading` is on it. So the `java-agent` capability's "durable fallback" survives for Web Start only through the *environment* (`JAVA_TOOL_OPTIONS`, which the child JVM inherits and which `dev-docs/java-toolkits.md` already measured as controlling the JEP 451 flag), never through the JNLP file. That raises the stakes of JEP 451 specifically for Web Start targets, and it is the reason the operator-facing remedy has to be written down rather than left as folklore.

## Goals / Non-Goals

**Goals**

- A Web Start application is served by the agent backend with the same fidelity as the same application launched directly.
- The agent's ability to run is not a decision the target's policy gets to make.
- A start-up failure is visible: never an `Error` in the target, never a silent no-handshake on the client.
- Regression coverage that reproduces both conditions without requiring OpenWebStart on the test host.

**Non-Goals**

- No support for applets. The `AppContext` work covers them incidentally, but nothing is claimed or tested for them.
- No change to the RPC protocol, the handshake format, the delivery package, or node shape. This change is invisible to consumers.
- No JavaFX or SWT adapter work; only the constraint this change imposes on them is recorded (see Risks).
- No attempt to make the agent work where dynamic agent loading is refused outright (JEP 451). That remains the `-javaagent` fallback's job.

## Decisions

### 1. Privileges via `Boot-Class-Path`, unconditionally

The agent JAR's manifest gains `Boot-Class-Path: platynui-agent.jar`. The JVM appends it to the bootstrap search before the agent class is loaded, so `platynui.agent.*` is defined by the bootstrap loader with a `null` `ProtectionDomain` — which means all permissions, regardless of the target's `Policy`. The relative name resolves against the JAR's own directory, which is why the artifact's version-less file name is load-bearing here too.

Verified: with this attribute the agent starts, publishes and serves inside the real OWS application; without it, it does not.

This does not widen PlatynUI's reach. Loading an agent into a JVM is already unrestricted code execution in that process — the reason JEP 451 exists at all. It is also precisely what the JNLP runtime does for itself: OWS starts the application JVM with `-Xbootclasspath/a:openwebstart.jar`.

Two follow-on fixes are forced by it, both already required for correctness anyway:

- `Agent.version()` reads its version resource through `Agent.class.getClassLoader()`, which is `null` for a bootstrap-defined class. It must go through the class (`Agent.class.getResourceAsStream("/…")`), which handles both cases. The existing comment there already anticipated "which class loader ends up defining the agent package"; the code did not.
- `AgentLog`'s static initializer must not throw when the debug property is denied, and `AgentRuntime.start` must catch `Throwable`.

**Alternatives rejected**

- *`AccessController.doPrivileged` around the privileged operations.* Useless: `doPrivileged` cannot grant a permission the code's own `ProtectionDomain` lacks, and it lacks all of them.
- *A grant in the target's user policy file* (`deployment.user.security.policy`, which ITW does consult). It works only for Web Start, only for cause 1, requires a file on every test host, and is exactly the kind of target-side configuration the capability's "without launch changes" promise exists to avoid.
- *Appending to the bootstrap search conditionally at runtime* (`Instrumentation.appendToBootstrapClassLoaderSearch` only when a `SecurityManager` is present, then loading the runtime reflectively from the bootstrap loader). This would avoid the JDK 9+ side effect below for the common case, but the condition cannot be evaluated when the decision must be made: under `-javaagent` the agent runs *before* the JNLP runtime installs its security manager, so it would take the unprivileged path and then break when the manager appears — the `accept()` on its already-open socket would start being denied. A wrong answer in the case that is hardest to debug is worse than a warning line in the case that is easy to explain.

### 2. `AppContext`-aware enumeration and dispatch

Two changes, both in the Swing adapter, both reached through `sun.awt.AppContext` reflectively (the agent compiles against Java 8's public API and must not link internals at compile time):

- **Enumeration** (`SwingTree.windows()`): iterate `AppContext.getAppContexts()` and take each context's window list, instead of calling `Window.getWindows()` on the current thread. The per-context list is what `Window.getWindows()` itself reads — `appContext.get(Window.class)`, a `Vector<WeakReference<Window>>` — so this reads the same source of truth, once per context rather than once for whichever context the caller happens to be in. The existing `isShowing()` filter continues to do the rest of the work: it is what already keeps `SharedOwnerFrame`s out, and it is what keeps the launcher's splash and download dialogs out, since they are not showing by the time the application's window is.
- **Dispatch** (`SwingDispatcher`): resolve the event queue of the `AppContext` that owns the element and post there, instead of `EventQueue.invokeLater`. The context's queue is reachable as `appContext.get(AppContext.EVENT_QUEUE_KEY)`; posting an `InvocationEvent` to it is what the JDK's own cross-context tooling does. The deadline logic in `ToolkitDispatcher.Calls` is unchanged and simply wraps a different submission target — so "abandoned at its deadline" keeps holding per context, and one wedged event queue cannot stall calls into another.

An element's context is a property of the element, not of the call: it is determined from the component (via its top-level `Window`) when the element is registered, and stored alongside the identity in `ElementRegistry`. Deriving it per call from the *agent's* thread is the bug being fixed and must not reappear in a different place, which is why the spec states it as a prohibition.

**A cross-world call answers with what it has.** `ui/windows` and `ui/at_point` concern every world rather than one element, so what a wedged world should do to them is a question the per-element rule does not settle. They ask each world in turn under the same deadline, and a world that misses it contributes nothing instead of failing the call — a frozen Web Start application must not make a healthy application in the same JVM disappear, and to a caller "this world did not answer" and "this world has no windows" are the same observation either way. When *no* world answers, the failure propagates: that is the frozen-JVM case the transport's containment promise is about, and it must keep presenting as an error rather than as an empty desktop.

**Alternatives rejected**

- *`SunToolkit.invokeLaterOnAppContext`.* Does exactly the right thing, but reaches one layer deeper than the two accessors used here (`getAppContexts`, `get`), which have been stable since 1.4 and are what the AWT code itself uses. No claim is made about `SunToolkit`'s own stability across releases — that was asserted here at first and nothing measures it; the fixture and the tests resolve `SunToolkit.createNewAppContext` without trouble on 8, 21 and 24. The reason to prefer the accessors is surface area, not observed churn.
- *Running the whole agent inside the application's `AppContext`* (creating its threads there). Attractive at first — everything downstream would then "just work" — but it binds the agent to *one* application in a JVM that may host several, and it changes which thread group the target's own code observes. An agent must not reshape the application's runtime.
- *Reading windows through the accessibility API instead* (which is AppContext-agnostic). That is the JAB path and its fidelity limits are why the agent exists.

### 3. The failure becomes visible on both sides

Agent side: `catch (Throwable)` in `AgentRuntime.start`, and a diagnostic channel that cannot itself throw. Client side: `await_readiness` in `crates/provider-java/src/agent/backend.rs` currently logs "the injected agent did not publish a handshake in time" at `debug!`. That is the exact line that would have named this bug on day one. It becomes `warn!` and names the target's own log as where the cause was printed, because that is where the agent's message goes and the operator has no way to guess that.

**This trades a signal, and the trade has to be paid for.** Measured on today's agent under a sandboxed target: the escaping `ExceptionInInitializerError` made the JVM answer the attach with `AgentInitializationException`, which the Rust side classifies as `AgentRefused` — a loud, typed error. Catching `Throwable` removes it: `agentmain` returns normally, the JVM answers `return code: 0`, and the only remaining evidence is a handshake that never appears. That is the right direction — an agent must not throw into the application it observes, and a stack trace in a foreign process's log is not a diagnostic channel we own — but it means the client-side warning is not cosmetic. It is the *only* signal left, which is why it is a requirement and not a nice-to-have, and why the spec's "agent rejected inside the target" scenario had to be narrowed to what the JVM itself refuses.

The same reasoning applies to `Agent.started`. It is set before the runtime start and never cleared, so a failed start makes every later injection into that JVM a silent no-op — the provider's three attach attempts then cannot possibly help, and a target that failed once for a transient reason stays unreachable for its whole life. Idempotence must key on "an agent is running here" (`AgentRuntime.current() != null`), not on "an attempt was made".

### 4. The regression fixture does not need OpenWebStart

Both conditions are reproducible with a plain JVM, which is what keeps them in the normal test lane:

- *Policy*: start the Swing fixture with `-Djava.security.manager` and a policy file granting the fixture's own code base `AllPermission` and nothing else anything. That is the shape of a signed `<all-permissions/>` JNLP — the application trusted, the agent not — and it is harsher than OWS only in irrelevant ways. Verified equivalent: the same denial (`getenv.PLATYNUI_AGENT_DIR`) appears in both.
- *AppContext*: start the fixture's UI inside a second `AppContext`, as `SunToolkit.createNewAppContext()` plus a thread group does — the same shape ITW produces. This belongs to the Swing fixture as an opt-in launch mode, not as its default, so the existing acceptance lane is untouched.

These go into the agent's live attach tests (`crates/java-agent/tests`), which already own "real JVM required" coverage, and the fixture mode into `apps/test-app-swing` per the `swing-test-app` capability.

## Risks / Trade-offs

- **[Class-data sharing is disabled for the target's own classes]** → Appending to the bootstrap class path makes the JVM print `Sharing is only supported for boot loader classes because bootstrap classpath has been appended` and lose CDS for application classes. Measured on Temurin 21 and 24; the boundary is not "9+" but "a JDK with a default CDS archive", which is 12 and up (JEP 341) — on 9 to 11 a target with no generated archive has nothing to lose and prints nothing. This is a real, if small, violation of "an agent must not change the behaviour of the application it observes": one stderr line and a slightly slower start-up, in a process an operator has deliberately chosen to instrument. Mitigation is disclosure rather than avoidance — documented in `dev-docs/java-toolkits.md` — because the conditional alternative is wrong for the `-javaagent` path (Decision 1).
- **[Bootstrap-defined agent classes cannot reference application-class-path types]** → Swing and AWT are unaffected: `java.desktop` is a boot-loader module, verified by running the bootstrap-loaded agent against a Swing application on JDK 21 and reading its full window tree — including the native handle through `sun.awt.windows.WComponentPeer#getHWnd`, which confirms `ModuleAccess.redefineModule` still opens `java.desktop` to the agent when the agent's module is the boot loader's unnamed one. `ToolkitDetector` is likewise immune, since it compares loaded-class *names* rather than referencing the types. The constraint binds the toolkits that are not part of the JDK — neither `javafx.*` nor `org.eclipse.swt.*` is resolvable from the boot loader, so their adapters must reach every type through a loader taken from the application side. That is out of scope here and is recorded where it will be acted on: [`provider-java-javafx`](../provider-java-javafx/design.md) decision 3 and [`provider-java-swt`](../provider-java-swt/design.md) decision 6, each with a matching risk entry. Both were written assuming direct references, so this is a correction to them, not an annotation — and it is the main reason decision 1 is worth writing down rather than just doing.
- **[The manifest's `Boot-Class-Path` value is coupled to the artifact's file name, silently]** → `Boot-Class-Path: platynui-agent.jar` resolves relative to the JAR's own directory. Rename the Gradle `archiveFileName`, or stage the artifact into `packages/provider-java` under a different name, and the entry simply matches nothing: the JVM says nothing, the agent falls back to system-loader loading, and *everything keeps working except in a policy-restricted target* — the one case nobody exercises locally. Mitigation is a test that pins the manifest attribute against the produced file name, and the staged name against it, rather than a comment asking people to remember.
- **[The policy fixture has an expiry date]** → JEP 486 permanently disabled the security manager in JDK 24: `-Djava.security.manager` with any value but `disallow` makes the JVM refuse to start, so decision 4's policy mode runs on JDK ≤ 23 only. Nothing is blocked today — the Swing fixture runs on the provisioned Java 8 — but what expires is the *only* automated check on decision 1, whose failure mode is silence (the `Boot-Class-Path` entry stops matching and everything keeps working except in a policy-restricted target). Hence the explicit pin at the launch site (task 1.2), so a toolchain bump says what happened. When the pin can no longer be held, the real-OpenWebStart harness in `scripts/webstart-repro/` is what is left, and task 6.5 stops being a cross-check and becomes the coverage.
- **[The structural-change hint across `AppContext`s — measured, and the guess was wrong]** → `SwingAdapter.watchStructuralChanges` registers a global `AWTEventListener`, and the expectation here was that it would hear only its own context, requiring one registration per world. Measured (`AppContextEventsTest`, headless JDK 21): **one registration hears every world**, because the listener list hangs off the single `Toolkit` instance rather than off an `AppContext`. Registering per world would install a listener per hosted application inside a foreign process and bump the counter once per listener for the same event, so the implementation registers once. What does still matter is *where from*: an agent thread in a multi-world JVM has no `AppContext`, and AWT calls that resolve one fail there, so the registration is posted to a world's event thread. The listener is also reachable at all only because bootstrap loading grants `listenToAllAWTEvents` — the existing `SecurityException` fallback stops triggering.
- **[`sun.awt.AppContext` is internal and was slated for removal]** → It still exists and is still what AWT itself uses through current JDKs, and it is unavoidable: there is no public API for cross-context window enumeration. Access is reflective and every failure degrades to today's behaviour (the agent's own context only) with a diagnostic, so a future JDK that removes it costs Web Start support rather than the agent. `ModuleAccess` already opens `java.desktop` internals for the native window handle, so no new opening mechanism is introduced.
- **[Identity churn across contexts]** → `ElementRegistry` gains a context per element. If that were derived lazily from the calling thread, identities would differ depending on who asked. It is captured at registration, from the element itself, and the liveness answer must keep working when the owning context is gone.
- **[The Windows acceptance lane must not move]** → The Swing fixture's default launch mode is unchanged, so the lane's 115 green tests are expected to stay exactly as they are. Any movement there is a regression in this change, not an update to the baseline.

## Migration Plan

No migration. The change is internal to the agent artifact and its client's logging; the handshake protocol version, the delivery package and every consumer-visible surface are untouched.

Deployment order matters only for developers testing it: the agent JAR must be restaged into the delivery package (`just install-provider-java`) or the installed package keeps serving the previous JAR while reporting the same dev version — the exact-version handshake cannot catch that.

Rollback is removing the manifest attribute and reverting the adapter, in one commit; nothing persists outside the artifact.

## Open Questions

- Should the agent report *which* toolkit worlds it found (an `appContexts` count in `agent/info`)? It is one line and would make "the agent sees no windows" self-diagnosing in the field. Deferred: it is a protocol addition, and the warning from Decision 3 may make it unnecessary.
- Whether the `AppContext` handling belongs in the neutral runtime rather than the Swing adapter, once a second toolkit adapter exists. JavaFX has the same problem in a different shape (`Platform.runLater` and a single FX thread, but multiple `Stage`s), so the *contract* is neutral — it is stated that way in the spec — while the mechanism stays with the adapter until a second one proves what they share.
