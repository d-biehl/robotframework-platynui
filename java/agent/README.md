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
  attributes, because which class loader defines the agent package depends on the injection path;
- **not** the file name: `platynui-agent.jar` is deliberately version-less, so the wheel, a manual
  `-javaagent:` command line and the discovery path can address it by a stable path.

Provider and agent versions must match **exactly**. Agents cannot be unloaded, so a mismatch aborts
the connection with a diagnostic naming both versions; delivery keeps them aligned by pinning
`platynui-provider-java == <same version>` in the `[java]` extra.
