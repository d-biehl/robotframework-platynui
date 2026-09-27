# Design

## Context

See proposal.md for the motivation and the specs for the required behavior. *Verified* marks what was read in the working tree at `18353c3`; *inferred* marks conclusions.

**The snapshot model (verified).**

- The runtime answers a query from a snapshot and lets the next query reuse it (`dev-docs/architecture.md` §9.3).
- When a query first touches a cached list of children, it checks each cached child with `UiNode::is_valid` and reads the whole list again if one is invalid (`crates/runtime/src/xpath.rs:548-561`, `:579-592`). A cached root is reused only while it is valid (`:320-342`).
- Revalidation stops at the first invalid child. All three providers list a process's windows before its application node (`crates/provider-windows-uia/src/provider.rs:308-336`, `crates/provider-java-jab/src/provider.rs:334-357`, `crates/provider-java/src/agent/backend.rs:385-414`). A process that has ended therefore already makes the desktop's list be read again through its windows. *Inferred:* an application node's own validity matters where the node itself is held: as the context of an evaluation (the cached root), as BareMetal's pinned root (`src/PlatynUI/BareMetal/__init__.py:267-274`), as a captured element (`Wait Until Gone`, `bool(node)`), and in the Inspector's rows (`apps/inspector/src/model/tree_data.rs:282-289`).

**The three application nodes (verified).**

- UI Automation's `ApplicationNode` holds a pid only (`crates/provider-windows-uia/src/node.rs:1762-1815`). The desktop stream creates one per pid for every enumeration (`provider.rs:292-336`). `ApplicationNode::orphan(pid)` caps the picker's ancestor chain. Its children are listed again on every call, by `EnumWindows` filtered by pid (`node.rs:1904-1912`).
- The Java Access Bridge's `JabAppNode` holds a pid and the client (`crates/provider-java-jab/src/node.rs:1319-1376`). It has no vmID, because the vmID belongs to each window. Its children are listed again on every call (`:1400-1423`). JAB calls go through the pump thread with a deadline (`client.rs:136`, `:241`), so they are no cheap way to check liveness.
- The agent's `AgentAppNode` holds its `Arc<AgentSession>` (`crates/provider-java/src/agent/app.rs:61-89`). The session has private `closed` and `degraded` flags (`session.rs:51`, `:56`) and a public `is_degraded()` (`:97`), but no accessor for `closed`. A session is closed when its handshake file is gone, and only during an enumeration (`backend.rs:228-242`).
- None of the three overrides `is_valid`, so all three report `true` (`crates/core/src/ui/node.rs:63-78`). Its documentation says that the default is for stubs only, and that a provider handing out nodes with a real lifetime owns the method.

**Process facts (verified).**

- `platynui_java_agent::jvm::process_is_alive(pid)` answers with `OpenProcess` + `GetExitCodeProcess` on Windows and with `kill(pid, 0)` on Unix (`crates/java-agent/src/jvm.rs:12-63`). It treats a process it cannot open as gone.
- UI Automation already reads a process's creation time with `GetProcessTimes` (`crates/provider-windows-uia/src/map.rs:440-446`). AT-SPI and JAB read start times through `sysinfo`, to the second (`crates/provider-atspi/src/process.rs:125`, `crates/provider-java-jab/src/process.rs:64`).
- `platynui-core` has no OS-specific dependency (`crates/core/Cargo.toml`). `provider-windows-uia` does not depend on `platynui-java-agent` (`crates/provider-windows-uia/Cargo.toml:17-35`).
- The in-flight change `application-process-attributes` (0/17 tasks) plans `crates/process` for the per-platform process readers, and rewrites the same three application nodes (its `design.md` D1, `:64-80`).
- AT-SPI has no synthetic application nodes. Its application nodes are the registry's accessibles, validated through `Accessible.GetRole` (`crates/provider-atspi/src/node.rs:452-458`).

**`PlatynUI.core` (verified).**

- `ContextBase.adapter` and `exists()` run `ensure_that(self._adapter_exists)` (`src/PlatynUI/core/context.py:185-247`, `:268-293`).
- A lookup ends in `adapter_factory.current.find_one`, which evaluates through `runtime.current.evaluate_single` against the runtime's shared snapshot (`adapter_factory.py:77-113`).
- Between retries, `ensure_that` sleeps and then calls `failed_func`, which is `self.invalidate` and drops only Python adapters (`core/ensure.py:129-173`). On the final failure `failed_func` does not run, because the timeout check comes first. `get_one`, `get_all` and `iter_all` call the factory without retries (`context.py:386-439`).
- `AdapterFactory` is an ABC with only `find_one` and `find_all` abstract (`adapter_factory.py:35-66`). A new method that is not abstract keeps the existing test stubs valid (`tests/PlatynUI/test_context.py:89-124`).
- `PlatynUI.core` is the object model for the future high-level library. Its keyword phase is pending, so no shipped keyword uses it (`dev-docs/python-migration-status.md:10`, `:311-317`).

## Goals / Non-Goals

**Goals:**

- Each application node reports whether the process it was created for is still running, on every platform that has such nodes.
- `PlatynUI.core` follows the snapshot protocol: it discards the snapshot before each retry and when a lookup gives up.

**Non-Goals:**

- BareMetal's clearing (maintainer decision). It keeps clearing after a lookup that found nothing, before `Query` and in its wait keywords, and it does not clear after a failed action.
- Checking BareMetal's root again inside its retry loop (maintainer decision: a later change).
- The rest of the per-platform process readers. That is `application-process-attributes`.
- Releasing snapshots from memory. That is `xdm-snapshot-release`.

## Decisions

### 1. An application node is valid while its process runs, not while it has windows

Validity is identity, as for every other node type: a UI Automation element is valid while it exists, and an AT-SPI accessible while it answers. An application node stands for a process, so it is valid while that process runs.

*Alternative rejected:* valid while the process has at least one window the provider would list. It costs `EnumWindows` and UI Automation's window filter, or a JAB or agent call, on every check. It would also turn invalid while an application swaps windows, for example when its main window closes as a dialog opens, and a pinned root would then be looked up again or fail for no reason.

### 2. The process is identified by pid and start time

At creation, the node records the process's start time together with the pid. Later checks compare them, so a pid the system gave to a new process does not count.

- **Windows:** `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, then `GetProcessTimes` (the creation time, in 100 ns units) and `GetExitCodeProcess`. A process object can outlive its process while another handle is open, so only `STILL_ACTIVE` counts as running, as `jvm::process_is_alive` already does.
- **Linux:** field 22 of `/proc/<pid>/stat` (`starttime`, in clock ticks since boot), read after the last `)`, because the command name in field 2 may contain spaces and parentheses. A zombie (state `Z` or `X`) counts as ended.
- **Other platforms** have no start-time reader. There the check falls back to whether a process with that pid exists (`kill(pid, 0)`). No application node exists on those platforms today.

The check answers one of three things:

- the process is running and is the same one;
- it has ended or been replaced;
- it cannot tell. That is the case when access is denied and the pid is still taken: on Windows `OpenProcess` fails with `ERROR_ACCESS_DENIED`; on Linux `/proc/<pid>/stat` cannot be read and `kill(pid, 0)` answers `EPERM`.

The node is invalid only for the second answer. A pid that no process has any more is known to be free even when the start time could not be recorded at creation: `ERROR_INVALID_PARAMETER` on Windows, `ESRCH` on Linux.

*Why "cannot tell" counts as valid:* if it counted as invalid, a desktop that shows a window of a process the user may not inspect would hold an application node that is invalid for good. Every query would then read the desktop's list of top-level elements again and drop the whole deeper snapshot, so the snapshot model would stop working while such a window is open (*inferred* from `xpath.rs:548-561`). Before this change such a node reported valid as well.

*Alternatives rejected:*

- The pid alone. It is simpler, but a reused pid would count as the old process, and the node's children would then list the new process's windows. A pinned root would move to another application without a sign.
- Holding a process handle for the node's lifetime. On Windows the pid cannot be reused while a handle is open, which makes the check a single call. But it keeps one kernel handle per application node, needs a `Drop` for each node type, and exists only on Windows. It also keeps every handle open while snapshots leak, until `xdm-snapshot-release`.

### 3. The identity lives in a new crate `crates/process`

`platynui-process` offers the capture at creation and the check later, with the platform code behind `cfg`. Its only dependencies are `windows` on Windows and `libc` on Unix. UI Automation, JAB and the Java provider depend on it.

*Alternatives rejected:*

- `platynui-java-agent::jvm`, next to `process_is_alive`. UI Automation would then depend on the agent's transport crate for a process check. `process_is_alive` also treats a process it cannot open as gone, which is the opposite of decision 2.
- `platynui-core`. Core has no OS-specific dependency and stays that way.
- A private copy in each provider. That would be three copies of the same platform code.

`application-process-attributes` plans the same crate for its per-platform readers. This change creates it with the identity part only, and that change adds its readers to it. Whichever change lands second rebases onto the other.

### 4. Each application node records its identity when it is created

UI Automation creates a new `ApplicationNode` for every process at every enumeration (`provider.rs:292-336`), and so do JAB (`provider.rs:334-357`) and the agent (`backend.rs:385-414`). Recording the identity in the constructor, `orphan` included, therefore costs one `OpenProcess` and `GetProcessTimes` per application and enumeration on Windows. That is small next to the COM calls that each top-level window's `is_valid` makes (*inferred*). Recording it lazily at the first `is_valid` would not do: by then the pid may already belong to another process.

### 5. The agent's application node also follows its session

`AgentAppNode::is_valid` is false while its session is closed or degraded, and otherwise answers as decision 2. `AgentSession` gains a public `is_closed()`. Every part of the check is local, so it does not call into the JVM. This brings the node in line with `java-provider`'s *Node validity is answered, not assumed*, whose text already covers it.

A degraded session can recover. The node then reports valid again, but a consumer that saw it as invalid has already looked its root up again and holds the node of the next enumeration. That is the intended effect: a root pinned to `/app:Application[@ProcessId=…]` moves to whatever serves the process then.

### 6. `PlatynUI.core` discards the snapshot on a miss, before each retry and when it gives up

- `AdapterFactory` gains `discard_snapshot()`, a no-op that is not abstract. `RuntimeAdapterFactory` implements it with `runtime.current.clear_cache()`.
- `RuntimeAdapterFactory.find_one` discards the snapshot when it finds nothing, and `find_all` when it finds nothing. This covers the lookups without retries (`get_one`, `get_all`, `iter_all`).
- `ContextBase.ensure_that` passes a `failed_func` that discards the snapshot and then invalidates the context. Every retry therefore reads the current UI, whether the element was missing or a condition such as "enabled" failed.
- When `ensure_that` gives up, whether it returns `False` or raises, `ContextBase.ensure_that` discards the snapshot once more, so the next call starts from the current UI.

A lookup that succeeds keeps the snapshot. The layering stays context → adapter factory → runtime.

*Alternative rejected:* discarding the snapshot before every lookup. It would give up the snapshot model's reuse for the object model, which is the layer that evaluates most often (one lookup per context level).

### 7. Tests where the behavior can be shown

- **Identity:** unit tests in `crates/process`:
  - the own process is running and the same;
  - a spawned child process is running while it lives and has ended after it was killed and reaped;
  - a recorded start time that does not match counts as replaced;
  - a pid that no process has cannot be recorded;
  - on Linux, `/proc/<pid>/stat` is parsed correctly for a command name with spaces and parentheses.
- **UI Automation:** a unit test with `ApplicationNode::orphan(pid)` for a spawned child process: valid while it runs, invalid after it ended.
- **Agent:** a unit test with a session that the test controls: closed or degraded makes the node invalid, and the check returns without a call.
- **JAB** cannot be unit-tested without a client. The live test `live_a_killed_jvm_leaves_no_valid_nodes` (`crates/provider-java/tests/live_fixture.rs:1271-1313`) is extended so that it also asserts the application node, for the JAB and the agent backend.
- **`PlatynUI.core`:** pytest with the existing stubs (`tests/PlatynUI/test_context.py`, `test_ensure.py`, `test_adapter_factory.py`). A stub factory counts `discard_snapshot`, and a fake runtime installed with `runtime.override` counts `clear_cache`.
- **Windows acceptance lane:** a pinned application root is looked up again after its process exits and fails with `RootNotFoundError`, and `Wait Until Gone` on the captured application node returns. This is covered for an application served by UI Automation (the egui test app) and for Swing served by JAB and by the agent. Each test launches its own instance, so that the suite's shared instance keeps running.

The mock provider cannot show any of this. Its tree is static, and `MockNode` keeps the default `is_valid` (`crates/provider-mock/src/node.rs:101-160`). This is the same reason why `tests/PlatynUI/test_baremetal_root_reuse.py:13-17` is a unit test.

## Risks / Trade-offs

- **[A pinned JAB application root does not move to the agent when the agent takes over the JVM in a later enumeration]** → The process still runs, so the `JabAppNode` stays valid. Its children come back empty, because the exclusions now give those windows to the agent (`crates/provider-java-jab/src/node.rs:1406-1409`). Within one enumeration the router enumerates again after attaching (`crates/provider-java/src/provider.rs:245-288`), and the agent-facing acceptance resource waits for `@Technology="JavaAgent"` before it pins the root (`tests/acceptance/swing/resources/testapp_agent.resource:34-39`). This is accepted as a limit and named in the Swing documentation of `dev-docs/java-toolkits.md`.
- **[Linux `/proc` mounted with `hidepid`]** → Another user's `/proc/<pid>` is then not visible at all. `kill(pid, 0)` still answers `EPERM` for a process that exists, so the check answers "cannot tell", and the node stays valid.
- **[More discarded snapshots in `PlatynUI.core`]** → Until `xdm-snapshot-release` lands, every discarded snapshot stays in memory. Implement that change first, or accept the growth in this layer, which no shipped keyword uses yet.
- **[A behavior change for suites that pin an application root]** → A query under a root whose process has ended now fails with `RootNotFoundError` instead of `ElementNotFoundError`. `RootNotFoundError` is a subclass that `logging-concept` introduced for exactly this case. A suite that expected the old error text for a dead application has to follow; the repo's own suites do not.
- **[`application-process-attributes` rewrites the same nodes]** → The overlap is small: a field and a constructor argument per node, and `is_valid`. Whichever change lands second rebases.

## Migration Plan

- **Behavioral where stated:** application nodes report invalid after their process ends, and `PlatynUI.core` discards the snapshot on a miss, between retries and when it gives up. No keyword, argument or return value changes.
- **Additive:** the crate `platynui-process` and `AgentSession::is_closed`.
- **Native rebuild:** yes. The new validity reaches Python through the extension (`UiNode.is_valid()`, `bool(node)`).
- **Sequence:** identity crate and its tests, then the three application nodes, then `PlatynUI.core`, then the acceptance tests and the documentation.
- **Rollback:** revert per commit. The crate is only additive, each node's `is_valid` reverts on its own, and the `PlatynUI.core` change is independent of the Rust changes.

## Open Questions

- The order relative to `application-process-attributes`. Deferrable: either change can land first and the other rebases (decision 3).
