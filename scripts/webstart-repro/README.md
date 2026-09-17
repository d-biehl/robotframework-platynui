# Java Web Start / OpenWebStart reproduction harness

Launches a Swing demo application through OpenWebStart and attaches a PlatynUI
agent JAR into it, so the agent can be checked against the two conditions a real
Web Start target imposes — a `JNLPSecurityManager` and a per-application AWT
`AppContext`.

This is a **manual** harness, deliberately outside the test lanes: it needs a
73 MB installer and a JVM download, which is not something CI can carry. The
automated coverage models both conditions with a plain JVM instead (see the
`java-agent-web-start` change). That model is only worth what its last
comparison against reality was worth — which is what this harness is for. Run it
when the agent's loading or threading changes, not on every commit.

## One-time setup

1. **OpenWebStart**, per user, no elevation:

   ```powershell
   $exe = "OpenWebStart_windows-x64_1_14_0.exe"   # github.com/karakun/OpenWebStart/releases
   & .\$exe -q -dir "$env:LOCALAPPDATA\Programs\OpenWebStart" -overwrite
   ```

   Its JVM manager fetches what the JNLP asks for (`<j2se version="1.8"/>` pulls
   a full Temurin 8 **JDK** into `~\.cache\icedtea-web\jvm-cache`). No Java 8 is
   needed on the host: anything that has to run in the target is compiled
   `--release 8` by a host **JDK 9 or newer** (`--release` does not exist before
   9, so a Java 8 JDK cannot serve as the host — `run.ps1` asks each candidate its
   version and refuses rather than picking one that cannot compile), and the
   attach itself is performed *by the target's own runtime*.

   That last point is not a convenience. An attach reply comes in two dialects —
   Java 8 answers a bare integer, JDK 9+ answers `return code: N` — and a newer
   JDK's `com.sun.tools.attach` client trips over an older target's reply with
   `AgentLoadException: Failed to load agent library: 0` **even when the agent
   loaded perfectly well**. PlatynUI's own transport parses both dialects
   (`crates/java-agent/src/attach/mod.rs`); this harness sidesteps the problem by
   matching versions.

2. **IcedTea-Web configuration** at `~\.config\icedtea-web\deployment.properties`,
   so an unsigned demo launches without trust prompts and logs what it denies:

   ```properties
   deployment.security.level=ALLOW_UNSIGNED
   deployment.security.askgrantdialog.show=false
   deployment.security.notinca.warning=false
   deployment.log=true
   deployment.log.headers=true
   ```

   `deployment.log=true` is the important one: the `Denying permission: (...)`
   lines are how a policy failure is read, and the agent's own `[PlatynUI agent]`
   output goes to the application's stderr, which the OWS console captures.

## Running it

```powershell
# The agent as built, into a Web Start application:
.\run.ps1 -Jar ..\..\java\agent\build\libs\platynui-agent.jar

# What the JVM's AppContexts actually look like, instead of the agent:
.\run.ps1 -Probe
```

`run.ps1` compiles the demo, serves it over loopback, launches it through
OpenWebStart, waits for the application JVM (the one OWS starts out of its JVM
cache — *not* the launcher JVM), attaches, and reports the handshake file, the
window list, and the permissions the run denied.

## What a healthy run looks like

```
HANDSHAKE PRESENT: {"protocol":1,...,"toolkits":["swing"],...}
windows -> {"windows":[{"className":"javax.swing.JFrame",...,"window":{"handle":...}}]}
```

Two failure shapes are worth recognising, because both once were the real state
of this agent:

- **Handshake absent, attach reports success.** The agent died inside the target.
  Look for `[PlatynUI agent] ... failed to start` and the `Denying permission`
  line above it. Cause: the agent's classes are subject to the target's policy —
  the JAR's `Boot-Class-Path` entry is missing or no longer matches the JAR's
  file name, in which case it silently falls back to system-loader loading.
- **Handshake present, `windows` empty.** The agent cannot see the application's
  `AppContext`. `-Probe` shows this directly: two contexts, the application's
  `JFrame` in the one the agent's threads are not in.

## Files

| Path | Purpose |
|---|---|
| `demo/WebStartDemo.java` | Swing demo that also reports which sandbox permissions it has; every probe is failure-tolerant, since a denial must not take the application down |
| `demo/demo.jnlp` | Unsigned, sandboxed, `<j2se version="1.8"/>` — the OWS JVM manager resolves that |
| `probe/ProbeAgent.java` | Diagnostic agent: dumps the JVM's `AppContext`s and each one's windows, as seen from an agent thread |
| `probe/Attach.java` | Attaches an arbitrary agent JAR by pid (the agent's own `AttachDriver` only loads itself) |
| `probe_rpc.py` | Minimal NDJSON-RPC client: handshake, then `ui/windows` |
| `run.ps1` | The driver |
