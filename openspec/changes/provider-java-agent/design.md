## Context

This is the "agent half" of the Java-toolkit map in [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md); the "detection half" is `java-app-classifier` (stage 1). It is captured as **forward design**: transport, discovery, and the data model are decided below; the remaining open questions (coordinate spaces, mixed-toolkit trees, window-handle resolution, threading details) are consolidated at the end and gated behind a spike. Treat the tasks as a roadmap that starts with that spike, not a settled plan.

The consent/fidelity axis is the reason this coexists with the native providers rather than replacing them:

```
              zero consent        one-time consent       per-launch consent
   Fidelity   ├ JAB (Win) ──────────────────────────────────────────────┤
     low      │ AT-SPI (SWT)      │                                       │
              │ AX (Mac)          │      agent (full, all platforms) ─────┤
     high     └───────────────────┴───────────────────────────────────────┘
```

## Goals / Non-Goals

**Goals:**

- Full-fidelity Java trees (real identity, correct bounds, working table cells) as ordinary PlatynUI nodes — same XPath, Inspector, picker.
- Keyword-free: routing is automatic via the classifier + `window_claims`; the user only writes locators.
- One provider (RPC client) across Windows/Linux/macOS, one agent artifact.

**Non-Goals:**

- Replacing JAB / native providers — they remain the zero/low-consent floor.
- Silent instrumentation — auto-attach is opt-in and off by default.
- A general bytecode-instrumentation framework — the agent reads accessibility/scene models, it does not rewrite app logic.
- **v1 covers Swing/AWT only** (decided). JavaFX and SWT adapters — and therefore mixed-toolkit trees — are later stages; the JavaFX-on-Linux gap stays open until the FX adapter ships. This holds v1 scope tight against the one toolkit where the fidelity pain is proven (JTable) and lets the wire/transport/lifecycle harden before a second toolkit is added.

## Decisions (proposed) and Open Questions

1. **One PlatynUI agent JAR, both entry points, toolkit self-detection.** The agent is a single PlatynUI-owned artifact with `premain` (launch) and `agentmain` (attach) entry points and `toolkit=auto` self-detection — from inside the JVM the toolkit is unambiguous (a `javax.swing` window hierarchy vs. a JavaFX `Stage` vs. an SWT `Display`), so the agent is the authoritative classifier that host-side detection defers to. The attach driver (a small main class around `VirtualMachine.attach`/`loadAgent`) ships **inside the agent JAR** — no separate attacher artifact — and reports "attach failed" and "agent rejected inside the target" (e.g. a sandboxed-JNLP `SecurityManager`) as distinct exit codes so failures stay diagnosable; `jattach` covers JRE-only test hosts. The agent also exposes point-hit-test/highlight endpoints for the picker — in-JVM hit-testing is trivial (`SwingUtilities.getDeepestComponentAt`) and closes the table-picker gap JAB cannot.

1a. **Control plane / data plane split; data plane = loopback TCP + handshake file.** Injection (`-javaagent` or Attach) is a **one-time control plane**; the running traffic flows over the agent's **own data plane**. The two must not be conflated — the JDK's attach mechanism is a command channel only (`load`, `properties`, dumps; short-lived sessions, one listener thread), and on Windows every attach is an `OpenProcess`+`CreateRemoteThread` into the target (EDR-visible), so it is used exactly once to load the agent and never for reads. (Prior art contrast: JAB *fuses* control and data into Win32 messaging — hidden-window rendezvous, then synchronous `SendMessage`/`WM_COPYDATA`/shared memory — which is why it is zero-config but Windows-only, blocks callers on a hung JVM, and needs UIPI workarounds. We keep its good idea, zero-config rendezvous, in portable form.)

    - **Transport:** the agent binds `127.0.0.1:0` (OS-chosen ephemeral port) with a plain `ServerSocket`. This is forced, not merely chosen: the agent must run on **Java 8** targets, where Unix domain sockets don't exist yet (JEP 380 = Java 16) and a named-pipe *server* is impossible in pure Java — loopback TCP is the only all-platform Java-8 server option. Framing: newline-delimited JSON-RPC 2.0; short client-side per-call timeouts, never open-ended socket reads. A **server-initiated notification frame** (a message without an `id`) is part of the wire from day one — v1 uses it only to publish the UI-generation counter (decision 7), so later `event_capabilities` need no protocol break. The handshake enforces an **exact provider↔agent version match** (decided): agents cannot be unloaded, so a mismatch is refused cleanly with a diagnostic naming the fix (restart the JVM with the current agent) — simplest rule, no silent drift; the wire can relax to semver ranges later if it ever hurts.
    - **Handshake file (rendezvous + auth + discovery in one):** on startup the agent writes `<per-user runtime dir>/platynui/agent-<pid>` (directory `0700`/owner-only) containing `{ port, token, toolkit, agentVersion, pid }`, and deletes it on shutdown. The provider discovers agents by reading these files and must present the random `token` in the connection handshake. This one mechanism solves: port collisions (the OS picks), discovery for **user-launched** `-javaagent` targets (no port argument needed; works even with `-XX:+DisableAttachMechanism`), authentication on multi-user hosts (loopback TCP alone is connectable by any local user — terminal servers are common in enterprise Java), and the classifier's "agent present in this JVM?" signal (file exists + ping). The token is deliberately **not** passed as a `-javaagent` argument (`/proc/<pid>/cmdline` is world-readable on Linux); the agent generates it and only the `0700` file carries it.

2. **Data model: component-tree spine + accessibility enrichment + virtual accessible subtrees — full surface from the start (decided).** The wire carries everything both levels offer (not a minimal set): direct properties incl. `Component.getName()`, client properties, model data (`TableModel` bulk reads), the complete state set, and the accessible view — the exact field list is pinned against the fixture in the spike, but the scope decision is "all of it". The agent reads **both** levels the JVM offers, in a fixed relationship:

    - **Spine = the toolkit instance tree** (`Window`→`Container`→`JComponent`; `Stage`→`Scene`→`Node`; `Display`→`Shell`→`Control`): complete, deterministic, carries geometry — and it is the only level that has `Component.getName()` (the classic automation id JAB could never see), direct model access (`TableModel` bulk reads instead of per-cell calls), and semantic actions. For JavaFX and SWT it is the only level at all (neither implements `javax.accessibility`).
    - **Enrichment = the accessible view per node** (`AccessibleContext` where present: accessibleName/role/description/states) — so locators written against JAB's `accessibleName` keep working unchanged, and apps that only annotated accessibility stay addressable.
    - **Virtual accessible subtrees where components end:** a `JTable`'s cells are not components but `AccessibleJTableCell` wrappers — *correct* in-process (name, `getCellRect` bounds, selection) — and custom-drawn components may expose structure only through accessibility; the agent grafts these as child nodes of the spine.

    Actions keep PlatynUI's philosophy: real input (pointer/keyboard) via correct bounds stays platform-level; the agent backs tree/attributes/bounds/hit-test plus selected patterns (Focusable via `requestFocus`, TextEditable via `setText`), mirroring the JAB provider's pattern surface.

2a. **Where PlatynUI adds value = the mapping layer.** The provider maps agent elements → normalized role/namespace, `native:*` attributes, patterns, and — crucially — **stable identity-based `RuntimeId`s**: the agent holds real Java object references, so unlike JAB's enumeration-index scheme, ids can be identity-stable. Element ids are **agent-assigned registry ids backed by weak references** — not Java `identityHashCode` (collision-prone, reused after GC) and not strong-ref caches (which would leak every component ever touched).

2b. **Coordinates: the agent emits physical desktop pixels (decided).** The conversion happens in the JVM (via `GraphicsConfiguration` transforms), where the toolkit's own scaling knowledge lives — Java 8 (already physical) and Java 9+ (per-monitor user-space) are normalized at the source, and the provider stays dumb. No provider-side calibration heuristic (the inverse of what JAB forced on us). The spike verifies the conversion on a HiDPI-scaled monitor across JDK 8/17/21.

3. **Routing: the registry ranks claims (decided).** The classifier (stage 1) gains an "agent present?" signal (the handshake file); every provider claims as today, and `window_claims` resolves the owner by an ordered **priority** (agent > JAB > native-for-Java) — generic providers ask "am I outranked on this window?" instead of "is it claimed by anyone?". Chosen over classifier-assigns-the-owner because ranking is robust against mid-session changes: when an agent appears in an already-running JVM (auto-attach, late `-javaagent` discovery), its higher-ranked claim re-routes the window on the next enumeration pass without any coordination protocol between classifier and providers.

4. **Port / instance discovery — DECIDED: the handshake file (decision 1a).** The agent binds an OS-chosen port and publishes it (with the auth token) in the per-user `agent-<pid>` file; the provider discovers agents by scanning that directory. No port arguments, no collisions across concurrent target JVMs, no repeated attach round-trips just to read a system property (which on Windows would mean a fresh `CreateRemoteThread` per poll). *Rejected alternatives:* fixed/configured ports (collide), port = f(pid) (fragile), system-property-via-Attach (couples discovery to the attach mechanism and is noisy on Windows).

5. **Injection paths — `-javaagent` durable, Attach best-effort; attach protocol implemented natively in Rust (decided).** Both load the same agent. The attach transport is a small PlatynUI platform crate speaking the JVM attach protocol directly (Unix: `.attach_pid<pid>` + `SIGQUIT` + the `/tmp/.java_pid<pid>` socket; Windows: `OpenProcess`/`WriteProcessMemory`/`CreateRemoteThread` invoking `JVM_EnqueueOperation`, reply over a client-created named pipe) — no bundled foreign binary, no JDK or `jattach` required on the test host; the `unsafe` Windows leg gets a focused review. Attach needs no launch change (Java Web Start). Per JEP 451 the Attach+`loadAgent` path warns on JDK 21 and will be default-disallowed in a future release (opt-in `-XX:+EnableDynamicAgentLoading`, settable via `JAVA_TOOL_OPTIONS` without editing the command line). So `-javaagent` is the durable path; Attach is a convenience with a documented expiry. Discovery/listing is unaffected by JEP 451 and is done natively (`hsperfdata` / the attach protocol).

6. **Auto-attach is opt-in.** `providers.java-agent.auto_attach` default **off**: off ⇒ agent used only where the user launched it with `-javaagent`; on ⇒ detected agent-less Swing/JavaFX JVMs are attached automatically (still keyword-free — consent is the one-time config flag, not a per-app call).

7. **Threading: multi-client, EDT-serialized, deadline + generation counter (decided).** The agent accepts **multiple concurrent provider connections** — the Inspector and a test run are separate PlatynUI processes and must not lock each other out. All reads marshal onto the toolkit thread (Swing EDT) anyway, which serializes them naturally; each call runs **with a deadline on the agent side too** — a bare `invokeAndWait` would let a hung EDT pin the RPC handler forever — and abandoned results are discarded, mirroring the JAB pump. The provider treats a hung/degraded agent like JAB treats a frozen JVM (bounded, no runtime hang, `DegradedTracker` discipline). The agent maintains a **UI-generation counter** (bumped on structural toolkit events) published over the reserved notification frame — a cheap invalidation hint for consumers, in v1 informational only.

8. **Native window handle: hybrid (decided).** For the WindowManager delegation of the window patterns, the agent first tries to read the native handle in-process (JDK internals — `sun.awt` peers on 8, their moved/jigsawed forms on 9+; what `--add-opens` that needs per version is spike work); when that fails, the provider falls back to PID + geometry/title matching against the native window list. Exactness when possible, portability always.

## Risks / Trade-offs

- [Attach path decays with future JDKs (JEP 451)] → `-javaagent` remains; documented, non-blocking.
- [Auto-attach is instrumentation] → off by default, one-time explicit opt-in, never silent.
- [Cross-platform agent lifecycle (spawn, port, teardown, multiple JVMs)] → the hardest engineering; the spike must exercise multiple concurrent targets.
- [Security: an open agent RPC port] → loopback-only bind **plus** the handshake-file token (any local user can connect to loopback on shared hosts); stale `agent-<pid>` files after a JVM crash must be detected (pid gone ⇒ ignore/clean).

## Migration Plan

New provider + agent artifact; additive. JAB and native providers unchanged and remain the default floor. Enabled via `providers.java-agent.enabled`; full auto behavior only with `auto_attach`. Requires `just build-native`. Rollback: disable the provider (config) — everything falls back to JAB/native.

## Decisions summary

| # | Topic | Decision |
|---|---|---|
| 1 | Agent artifact | one PlatynUI JAR, `premain`+`agentmain`, `toolkit=auto`, attach driver in-jar, picker endpoints |
| 1a | Transport | control/data split; data = loopback TCP + NDJSON-RPC; handshake file (port+token, `0700`); notification frame reserved; **exact version match** |
| 2 | Data model | spine (instance tree) + accessibility enrichment + virtual subtrees; **full field surface** |
| 2a | Mapping | provider maps to `UiNode`; identity-stable RuntimeIds via agent-assigned weak-ref registry ids |
| 2b | Coordinates | **agent emits physical desktop pixels** (converts in-JVM) |
| 3 | Routing | **`window_claims` ranks** (agent > JAB > native) |
| 4 | Discovery | handshake file (subsumes port allocation + auth + agent-present signal) |
| 5 | Injection | `-javaagent` durable + Attach; **attach protocol reimplemented in Rust** (no foreign binary / JDK on host) |
| 6 | Auto-attach | opt-in, default off |
| 7 | Threading | **multi-client**, EDT-serialized, agent-side deadline, **UI-generation counter** |
| 8 | Window handle | **hybrid**: in-JVM internals first, PID+geometry fallback |
| — | v1 scope | **Swing/AWT only**; JavaFX/SWT (and mixed trees) are later stages |
| — | Events | wire frame reserved; push deferred past v1 |

## Spike verification items (approach decided; numbers/mechanics to confirm)

- **Coordinate conversion** (2b): confirm the in-JVM physical-pixel conversion on a HiDPI-scaled monitor across JDK 8 / 17 / 21.
- **Window-handle internals** (8): which `sun.awt`/FX internals expose the native handle per JDK, and the `--add-opens` each needs on 9+ — before the PID+geometry fallback kicks in.
- **Element-model field list** (2): pin the exact attribute/pattern payload against the Swing fixture (the *scope* is "full", the field list is to enumerate).
- **Concurrency** (7): two provider connections (Inspector + test run) against one agent; deadline abandons a wedged EDT job cleanly.
- **Attach protocol** (5): the native Rust attach against a running JVM on each OS, incl. the `unsafe` Windows `CreateRemoteThread` leg and its review.
- **Packaging** (1a): agent `-target 8` in the wheel; exact-version handshake refuses a stale agent with an actionable diagnostic.
