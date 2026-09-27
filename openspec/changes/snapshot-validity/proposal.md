# Proposal

## Why

The runtime answers each query from a snapshot of the UI. The next query may reuse that snapshot, and `UiNode::is_valid` is what tells it that an element has gone (`dev-docs/architecture.md` §9.3). The caller clears the snapshot when it did not find what it looked for. Two parts of that protocol do not hold today.

- **Three node types never become invalid.** The synthetic application nodes of UI Automation (`crates/provider-windows-uia/src/node.rs:1866`), the Java Access Bridge (`crates/provider-java-jab/src/node.rs:1378`) and the Java agent (`crates/provider-java/src/agent/app.rs:91`) do not override `is_valid`, so they report valid forever (`crates/core/src/ui/node.rs:76`). Every acceptance resource pins its suite root to such a node (`Set Root /app:Application[@ProcessId=${pid}]`, e.g. `tests/acceptance/qt/resources/testapp.resource:59`). BareMetal reuses a root while it reports valid (`src/PlatynUI/BareMetal/__init__.py:267-274`). A root whose application has ended is therefore never looked up again, and a query under it waits until its timeout and fails with `ElementNotFoundError`. The agent's application node also breaks the existing `java-provider` requirement *Node validity is answered, not assumed*, which forbids valid-by-default. No suite shows this, because the Swing close test switches to a desktop-absolute selector after the fixture exits (`tests/acceptance/swing/window.robot:41-49`).
- **`PlatynUI.core` never refreshes the snapshot.** Its lookups evaluate against the runtime's shared snapshot (`src/PlatynUI/core/adapter_factory.py:93`, `:112`). Between retries its `ensure_that` loop drops only the Python adapter (`src/PlatynUI/core/context.py:287-293`), so every retry asks the same snapshot again. `exists()` can therefore time out for a window that is on the screen. No code under `src/PlatynUI/core` calls `clear_cache()`.

## What Changes

- **Application nodes answer their validity.** An application node is valid while the process it was created for is running. It records the process's start time together with the pid, so a pid that the system gives to a new process does not count as the same application. On Windows the start time is the process creation time; on Linux it is the start time in `/proc/<pid>/stat`. When the start time cannot be read, for example because access to the process is denied, the node reports valid, as today. Otherwise a node that stays invalid for good would make every query read the desktop's list of top-level elements again. The agent's application node is also invalid once its agent session is closed or degraded, as `java-provider` already requires for agent-served nodes.
- **A shared process identity.** Capturing a process's identity and checking it later is one helper in a new small crate `platynui-process` (`crates/process`), used by all three providers. The in-flight change `application-process-attributes` plans this crate for its per-platform process readers; this change creates it with the identity part only.
- **`PlatynUI.core` refreshes the snapshot.** A context that did not find its element, or whose check failed, discards the runtime's snapshot before it asks again. A lookup that gives up also leaves no snapshot behind, so the next call reads the current UI.
- **Docs follow the model.** BareMetal's user documentation says that every query is answered "never [against] a stored snapshot" (`src/PlatynUI/BareMetal/__init__.py:514-517`); it now describes the snapshot and when the library reads the UI again. `tests/PlatynUI/test_baremetal_root_reuse.py:13-17` names `window.robot` as the real proof that a dying root is looked up again, which it is not; the new acceptance tests take that role.
- **Deliberately unchanged (maintainer decision):**
  - BareMetal keeps clearing the snapshot only after a lookup that found nothing, before `Query`, and in its wait keywords. It does not clear after a failed action.
  - BareMetal does not check the root again inside its retry loop. That is a separate, later change.
- **Behavior change, not breaking:** a query under a pinned application root whose process has ended now looks the root up again. With a pid-based root selector it fails with `RootNotFoundError`, which names the root, instead of `ElementNotFoundError` for the target. With a name-based root selector it follows a restarted application. `bool(node)` and `node.is_valid()` of an application node are `False` once its process has ended.

## Capabilities

### New Capabilities

- `application-node-validity`: when a synthetic application node reports itself valid, across the providers that create one (UI Automation, the Java Access Bridge, the Java agent).
- `ui-context-lookup`: how a `PlatynUI.core` context looks up its element, and when it reads the UI again instead of asking the same snapshot.

### Modified Capabilities

None. The agent's application node is brought in line with the existing `java-provider` requirement *Node validity is answered, not assumed*, whose text does not change. BareMetal's existing requirement *A root whose element died is looked up again* (`baremetal-selector-resolution`) becomes true for application roots and gets its first real-provider test; its text does not change either.

## Impact

- **Rust crates:**
  - new `crates/process` (package `platynui-process`): process identity capture and check, for Windows and Linux. It is added to AGENTS.md's crate list and to `windows_rust_packages` and `macos_rust_packages` in the justfile.
  - `crates/provider-windows-uia` (`ApplicationNode`), `crates/provider-java-jab` (`JabAppNode`), `crates/provider-java` (`AgentAppNode`, plus an `is_closed` accessor on `AgentSession`).
- **Python:** `src/PlatynUI/core/adapter_factory.py` (a hook that discards the snapshot, a no-op by default) and `src/PlatynUI/core/context.py` (the retry and give-up paths). BareMetal changes its documentation only.
- **Tests:**
  - Rust unit tests for the identity helper (a spawned child process), for `ApplicationNode` and for `AgentAppNode` with a fake session.
  - The live Java tests (`crates/provider-java/tests/live_fixture.rs`) cover the JAB and agent application nodes.
  - pytest cases with stubs for `PlatynUI.core`.
  - New Windows acceptance tests for an application root after its process has ended, for UI Automation and Swing.
  - The mock provider cannot show any of this: its tree is static and its nodes are always valid.
- **Native rebuild:** yes. Application-node validity reaches Python through the extension.
- **Platforms:**
  - Windows: all three application node types (UI Automation, JAB and the agent are Windows providers per the README).
  - Linux: only the identity helper, which the agent's application node uses once `java-provider-linux` makes the Java provider portable. AT-SPI has no synthetic application nodes; its application nodes are the registry's accessibles, and they already answer `is_valid` over D-Bus.
  - macOS: nothing.
- **Coordination:**
  - `application-process-attributes` (0/17 tasks) plans `crates/process` and rewrites the same three application node types. Whichever change lands second rebases onto the other.
  - Until `xdm-snapshot-release` lands, every discarded snapshot stays in memory. `PlatynUI.core` discards more often after this change, so implementing `xdm-snapshot-release` first is preferred.
