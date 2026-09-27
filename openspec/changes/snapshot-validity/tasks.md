# Tasks

Tests come first in each group. The mock provider cannot show application-node validity or a refreshed snapshot (its tree is static and its nodes are always valid), so the Rust behavior is proven by unit tests with spawned processes and by live tests. The Robot Framework proof runs in the acceptance lanes.

## 1. Failing tests first — process identity

- [ ] 1.1 Create `crates/process` (package `platynui-process`) with tests only (design decision 3). Cover:
  - the own process is recorded and checks as running and the same;
  - a spawned child process checks as running while it lives, and as ended once it has been killed and reaped;
  - an identity with the right pid but another start time checks as replaced;
  - recording a pid that no process has fails;
  - on Linux: `/proc/<pid>/stat` is parsed from after the last `)`, for a command name that contains spaces and `)`; a zombie counts as ended; `ESRCH` means ended and `EPERM` means "cannot tell";
  - on Windows: `ERROR_ACCESS_DENIED` means "cannot tell" and `ERROR_INVALID_PARAMETER` means ended. The parts that need a denied process are tested through the function that maps the error codes.

  Declare the dependencies (`windows` on Windows, `libc` on Unix). Add the crate to AGENTS.md's crate list, to the crate tree in `dev-docs/architecture.md` §2, and to `windows_rust_packages` and `macos_rust_packages` in the justfile. Verify that `just test-crate platynui-process` fails to compile.

## 2. Process identity

- [ ] 2.1 Implement the capture and the three-way check (design decision 2). Verify that 1.1 passes with `just test-crate platynui-process` on Windows, and with the same recipe on a Linux host.

## 3. Failing tests first — application nodes

- [ ] 3.1 UI Automation (`crates/provider-windows-uia`), a unit test with `ApplicationNode::orphan(pid)` for a spawned child process without a window: valid while it runs, invalid after it was killed. Also: a node created for a pid that no process has is invalid from the start. Verify that it fails today with `just test-crate platynui-provider-windows-uia`.
- [ ] 3.2 Agent (`crates/provider-java`), unit tests with a session the test controls: the node is invalid while the session is closed or degraded, valid again once the degraded state ends, and the check returns without a call, even when every call of the session fails. Verify that they fail today with `just test-crate platynui-provider-java`.
- [ ] 3.3 Extend `live_a_killed_jvm_leaves_no_valid_nodes` (`crates/provider-java/tests/live_fixture.rs:1273`) so that it also asserts that the JVM's application node reports invalid after the kill. Run it for the agent backend and, with the agent disabled, for the JAB backend. It is `#[ignore]` and runs in `just test-acceptance-windows`. Verify on Windows that it fails today.

## 4. Application nodes

- [ ] 4.1 `ApplicationNode` records its identity in `build` (so `new` and `orphan` both do), and `is_valid` answers from it (design decisions 2 and 4). Verify that 3.1 passes.
- [ ] 4.2 `JabAppNode` records its identity in `build`, and `is_valid` answers from it. Verify that the JAB part of 3.3 passes on Windows.
- [ ] 4.3 `AgentSession` gains `is_closed()`. `AgentAppNode` records its identity in `new`, and `is_valid` is false while its session is closed or degraded, and otherwise answers from the identity (design decision 5). Verify that 3.2 and the agent part of 3.3 pass.
- [ ] 4.4 Rebuild the extension (`just build-native`), and confirm by hand on Windows: `Runtime().evaluate('/app:Application')` returns nodes that report valid, and after a spawned application exits, the node that was taken for it reports `is_valid() == False` and `bool(node) == False`.

## 5. Failing tests first — `PlatynUI.core`

- [ ] 5.1 pytest in `tests/PlatynUI/`, with a stub factory that counts `discard_snapshot` and a fake runtime installed with `runtime.override` that counts `clear_cache` (design decision 6). Cover the `ui-context-lookup` scenarios:
  - an element that appears on the third attempt is found, and the snapshot was discarded before each retry;
  - a condition such as "enabled" that fails at first is retried after the snapshot was discarded;
  - `exists()` that gives up at its timeout discards the snapshot, and a later `exists()` finds the element that appeared meanwhile;
  - `get_one`, `get_all` and `iter_all` that find nothing discard the snapshot and report as today;
  - two successful lookups discard nothing;
  - `RuntimeAdapterFactory.find_one` discards when it finds nothing and not when it finds something, and so does `find_all`.

  The existing stubs (`tests/PlatynUI/test_context.py:89-124`) keep working unchanged. Verify that the new cases fail today with `just test-python`.

## 6. `PlatynUI.core`

- [ ] 6.1 Add `AdapterFactory.discard_snapshot()` (a no-op that is not abstract) and implement it in `RuntimeAdapterFactory` with `runtime.current.clear_cache()`. `find_one` and `find_all` discard the snapshot when they find nothing. Verify the factory cases of 5.1.
- [ ] 6.2 `ContextBase.ensure_that` passes a `failed_func` that discards the snapshot and then invalidates the context, and discards once more when `ensure_that` returns `False` or raises. Verify that 5.1 passes with `just test-python`, and that `just mypy` is clean.

## 7. Acceptance tests (Windows lane, real providers)

Follow the `robot-test-style` skill. Each test launches its own instance, so that the suite's shared instance keeps running.

- [ ] 7.1 An egui suite, `tests/acceptance/egui/app_root_after_exit.robot`, without a platform tag:
  - launch a test-app instance and pin its application node with `Set Root    /app:Application[@ProcessId=${pid}]    scope=LOCAL`;
  - end the process;
  - under a short `LOCAL` query timeout, a relative target fails with `RootNotFoundError`, naming the root;
  - `Wait Until Gone` on the application node, captured before the exit, returns.

  On Windows this exercises UI Automation's `ApplicationNode`. On the Linux lanes it exercises AT-SPI's application accessible, which already behaves this way. Verify that it fails today on the Windows lane, with `ElementNotFoundError` after the timeout.
- [ ] 7.2 The same two checks for Swing, served by JAB (`tests/acceptance/swing/`, based on `testapp.resource`) and by the agent (based on `testapp_agent.resource`). Verify that they fail today on the Windows lane.
- [ ] 7.3 Rewrite the docstring of `tests/PlatynUI/test_baremetal_root_reuse.py:13-17`: the real proof that a dying root is looked up again is now 7.1 and 7.2, not `window.robot`. Verify by reading.

## 8. Documentation

- [ ] 8.1 BareMetal's library introduction (`src/PlatynUI/BareMetal/__init__.py:514-517`): replace "never a stored snapshot" with a short, user-facing description. A keyword may reuse what an earlier keyword read, and the library reads the UI again after a lookup that found nothing, before `Query`, and in every wait. So a suite that expects a changed UI waits for it. Verify with the rendered libdoc of `PlatynUI.BareMetal`.
- [ ] 8.2 `crates/core/src/ui/node.rs:63-78` (`UiNode::is_valid`): name the application nodes as nodes with a real lifetime. `dev-docs/architecture.md` §9.3: add that `PlatynUI.core` follows the snapshot protocol too. `dev-docs/python-library-design.md:6049-6051`: the note becomes the implemented behavior. `dev-docs/java-toolkits.md`: the JAB-to-agent handover limit of the design's risks. Verify by reading.

## 9. Verification

- [ ] 9.1 Run `just check`, `just test` and `just test-python`, then `just build-native`. Verify that everything is green.
- [ ] 9.2 On Windows, run `just install-provider-java`, then `just test-acceptance-windows`. Verify that everything is green, including 3.3, 7.1 and 7.2. Then verify that `uv run --no-sync robotcode results log --level WARN --execution-messages` shows no PlatynUI warning. Record the outcome in this task.
- [ ] 9.3 On a Linux host, run `just test-crate platynui-process`, `just headless=true test-acceptance-x11` (7.1 against AT-SPI) and `just cross-target-checks`. Verify that everything is green. Record the outcome in this task.

## 10. Commit (only when the user asks)

- [ ] 10.1 Commit in reviewable steps, each lint-clean on its own: the process crate; the application nodes (one commit per provider or one for all three); `PlatynUI.core`; the acceptance tests and documentation. Subjects ≤ 72 characters. The commit for the application nodes names the behavior change in its body: a query under an application root whose process has ended fails with `RootNotFoundError`.
