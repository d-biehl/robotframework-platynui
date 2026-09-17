# PlatynUI Java agent

The artifact PlatynUI loads **into a target JVM** so it can read the toolkit's own in-process model
— the approach that gets past what native accessibility leaves on the table for Java UIs (see
[`dev-docs/java-toolkits.md`](../../dev-docs/java-toolkits.md)). It is the foundation of the
OpenSpec change `java-agent-core`; the per-toolkit tree readers land with the adapter changes
(`provider-java-swing` first).

This is a **product**, not a fixture — which is why it lives under `java/` and not under `apps/`,
where the Java test applications are.

## What it is

One JAR with both JVM entry points ([`Agent`](src/main/java/platynui/agent/Agent.java)):

| Attribute | Class | Injection path |
|---|---|---|
| `Premain-Class` | `platynui.agent.Agent` | `-javaagent` at launch — the durable fallback |
| `Agent-Class` | `platynui.agent.Agent` | attach into a running JVM — the **primary** path |
| `Main-Class` | `platynui.agent.AttachDriver` | convenience attach driver for JDK hosts |

Attach is primary because Java applications are launched by scripts, installers or Web Start (the
launch line is typically not PlatynUI's to change), and the Inspector's core use is looking into an
application that is *already running*. PlatynUI speaks the JVM attach protocol natively, so the
test host needs **no JDK and no bundled foreign binary**; `AttachDriver` is only for hosts that
happen to have a JDK and for diagnosing the native path.

The agent asks for **no instrumentation capabilities** (`Can-Redefine-Classes` and
`Can-Retransform-Classes` are both `false`): it reads accessibility and scene models, it never
rewrites application logic.

It does use the instrumentation handle for two things, both read-only in effect:

- `getAllLoadedClasses()` for toolkit detection — deliberately not `Toolkit.getDefaultToolkit()`,
  which would *initialise* AWT in an application that never had it.
- `redefineModule()` to open the `java.desktop` packages the toolkit adapters read
  ([`ModuleAccess`](src/main/java/platynui/agent/ModuleAccess.java)). Measurement says the native
  window handle needs both `java.awt` and `sun.awt.windows` opened on Java 9+, and the documented
  remedy — `--add-opens` on the command line — is unavailable to a design built on attaching to a
  JVM somebody else launched. JEP 261 gave instrumentation agents this power for exactly that reason,
  so no launch flags are needed on any JDK.

The **Swing/AWT adapter** ([`SwingAdapter`](src/main/java/platynui/agent/SwingAdapter.java)) is
installed only once toolkit detection has seen Swing or AWT, and serves the tree over `ui/*` methods:
`ui/windows`, `ui/children`, `ui/element`, `ui/at_point`, `ui/focus`, `ui/window_handle`. Notably
absent: any text write (text is typed via synthesized keyboard input) and any highlight (drawing is
the platform's job — what an out-of-process bridge lacks for a table cell is bounds, not a way to
draw).

"The toolkit thread" is resolved **per element**, not per JVM. AWT partitions a JVM into
`AppContext`s with a window list and an event queue each, and a Web Start or applet runtime creates
one per hosted application — so enumeration takes the union across them
([`AppContexts`](src/main/java/platynui/agent/AppContexts.java)) and every call is dispatched to the
queue of the context that owns the element it concerns, recorded when the element's id is handed
out. Using the agent thread's own context instead is how an agent sees an empty desktop in exactly
the applications it exists for.

The tree it serves is the **instance tree as the spine**, with model-derived structure grafted where
components end. A `JTable` is the case where that matters most: it has no child components, and its
accessible projection is a flat, row-major list of cells — so the adapter reads the model instead and
reports `Table` → row → cell, each row and cell an interned value object with its own identity,
rectangle and selection state. `SwingTree` builds and interns them, `SwingElement` describes them.

## Dependencies: none, deliberately

The agent is loaded into a foreign process, so every jar on its classpath would be a jar the target
application did not ask for. JSON framing and the RPC server are hand-rolled against `java.base`.

## Build

```sh
just build-java-agent    # → build/libs/platynui-agent.jar
```

Same self-bootstrapping toolchain story as the Java fixtures: any `java` 8+ on `PATH` is the only
prerequisite. The wrapper *client* runs on it, the Gradle *daemon* JVM (Temurin 21) comes from the
committed `gradle/gradle-daemon-jvm.properties`, and the JDK 21 compile toolchain is provisioned by
the Foojay resolver. The first build needs **network access** (cached user-level, shared with the
fixtures under `apps/`).

The product targets **Java 8 bytecode** (`--release 8`), and that is not nostalgia: enterprise Swing
applications still run on 8, and an agent that cannot load there is useless for them. `-Xlint:all`
plus `-Werror` are on — this code runs inside somebody else's process.

`just build-native` deliberately does **not** build this JAR and stays JDK-free. A missing JAR is a
runtime diagnostic on the discovery path ("install `robotframework-platynui[java]`"), never a build
failure. Only the release/wheel recipes and the lanes that actually exercise the agent treat it as a
hard prerequisite.

## Versioning

The version in [`gradle.properties`](gradle.properties) must stay in lockstep with the workspace
version in `Cargo.toml` / `pyproject.toml` — like every other per-package version literal in this
repo. It travels three ways, all from that one value:

- the JAR manifest (`Implementation-Version`), for humans and tooling;
- a generated resource `platynui/agent/version.properties`, which is what `Agent.version()` reports
  and what the handshake file carries — read as a resource rather than from package manifest
  attributes, because which class loader defines the agent package depends on the injection path.
  That is no longer an incidental remark: the manifest's `Boot-Class-Path` puts the agent's classes
  on the **bootstrap** loader (see below), for which `getClassLoader()` is `null`, so the resource
  is read through the class. For a bootstrap-defined class that resolves via the *system* class
  loader, which works only because every injection path also appends the JAR to the system class
  path — a JAR placed on the bootstrap search alone reports `"unknown"`, which is a property of the
  JDK's resource lookup rather than a defect to fix;
- **not** the file name: `platynui-agent.jar` is deliberately version-less, so the wheel, a manual
  `-javaagent:` command line and the discovery path can address it by a stable path. The manifest's
  `Boot-Class-Path: platynui-agent.jar` resolves **relative to the JAR's own directory**, so that
  name is now load-bearing in a second way: rename the build output or stage the artifact under
  another name and the entry matches nothing, the JVM reports nothing, and the agent silently loses
  the privileges a sandboxed target denies it. `checkAgentManifest` fails the build on a mismatch,
  and the delivery test pins the shipped name.

Provider and agent versions must match **exactly**. Agents cannot be unloaded, so a mismatch aborts
the connection with a diagnostic naming both versions; delivery keeps them aligned by pinning
`platynui-provider-java == <same version>` in the `[java]` extra.

## Privileges in the target: `Boot-Class-Path`

The manifest carries `Boot-Class-Path: platynui-agent.jar`, which loads the agent's classes through
the **bootstrap** class loader. That is not an optimisation, it is what makes the agent work at all
in a target with a restrictive security policy.

An attached agent JAR is appended to the target's *system* class path, so its protection domain is
an ordinary file code source holding only the base policy's grants. Under a Java Web Start
`SecurityManager` the agent is then denied its very first call — measured against OpenWebStart 1.14:

```text
Denying permission: ("java.lang.RuntimePermission" "getenv.PLATYNUI_AGENT_DIR")
```

while the attach reports success, so the failure is silent. A bootstrap-defined class has a `null`
protection domain and therefore all permissions, regardless of the target's `Policy`. This widens
nothing: loading an agent into a JVM is already unrestricted code execution in that process — the
reason JEP 451 exists — and it is what the JNLP runtime does for its own code.

Three things follow, all of them easy to trip over:

- **A bootstrap-defined class can only resolve what the boot loader defines.** `java.desktop` is
  such a module, so `javax.swing` and `sun.awt` are fine and the Swing adapter is unaffected
  (verified on JDK 8 and 21, native window handle included). Anything on the *application* class
  path is not: neither `javafx.*` nor `org.eclipse.swt.*` can be referenced directly or found by a
  bare `Class.forName` from agent code — those adapters must reach every type through a loader
  taken from the application side.
- **The target loses class-data sharing for its own classes** — on any JDK that ships a default CDS
  archive, which is 12 and up (JEP 341); measured on Temurin 21 and 24 — and prints `Sharing is only
  supported for boot loader classes because bootstrap classpath has been appended`. A real, small
  cost imposed on the observed application; disclosed rather than avoided, because the conditional
  alternative cannot work under `-javaagent` (the agent runs before a security manager exists, so
  the condition cannot be evaluated when the decision has to be made).
- **The JAR's file name is part of the contract** — see Versioning.
