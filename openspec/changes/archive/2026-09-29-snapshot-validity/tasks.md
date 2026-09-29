# Tasks

Tests come first in each group. The mock provider cannot show application-node validity or a refreshed snapshot (its tree is static and its nodes are always valid), so the Rust behavior is proven by unit tests with spawned processes and by live tests. The Robot Framework proof runs in the acceptance lanes.

## 1. Failing tests first — process identity

- [x] 1.1 Create `crates/process` (package `platynui-process`) with tests only (design decision 3). Cover:
  - the own process is recorded and checks as running and the same;
  - a spawned child process checks as running while it lives, and as ended once it has been killed and reaped;
  - an identity with the right pid but another start time checks as replaced;
  - recording a pid that no process has fails;
  - on Linux: `/proc/<pid>/stat` is parsed from after the last `)`, for a command name that contains spaces and `)`; a zombie counts as ended; `ESRCH` means ended and `EPERM` means "cannot tell";
  - on Windows: `ERROR_ACCESS_DENIED` means "cannot tell" and `ERROR_INVALID_PARAMETER` means ended. The parts that need a denied process are tested through the function that maps the error codes.

  The child process is the test binary itself, re-executed into an ignored test that only waits (design decision 7); no application that ships with the operating system. Declare the dependencies (`windows` on Windows, `rustix` on Unix). Add the crate to AGENTS.md's crate list, to the crate tree in `dev-docs/architecture.md` §2, and to `windows_rust_packages` and `macos_rust_packages` in the justfile. Verify that `just test-crate platynui-process` fails to compile.

## 2. Process identity

- [x] 2.1 Implement the capture and the three-way check (design decision 2). Verify that 1.1 passes with `just test-crate platynui-process` on Windows, and with the same recipe on a Linux host.

  Progress (2026-09-28): implemented, and revised after review: a process that exited with code 259 is told apart by waiting on its handle, a zombie leader whose threads run on has not ended, and other Unix systems answer "cannot tell" for a pid that is taken. On Windows `just test-crate platynui-process` passes 9 of 9 (the waiting child is skipped, as intended). clippy with `-D warnings` is clean for the host, `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`. The run on a Linux host is still open; it is part of 9.3.

  Outcome (2026-09-29), Linux host: `just test-crate platynui-process` passes 29 of 29, the identity tests of 1.1 among them. That was the maintainer's run on a clean worktree of `009595b`, which holds this change (`8f6fc02`, `94f5f97`). See 9.3.

## 3. Failing tests first — application nodes

- [x] 3.1 UI Automation (`crates/provider-windows-uia`), a unit test with `ApplicationNode::orphan(pid)` for a spawned child process without a window: valid while it runs, invalid after it was killed. The child is the test binary, re-executed like the test window of that module but without a window. Also: a node created for a pid that no process has is invalid from the start. Verify that it fails today with `just test-crate platynui-provider-windows-uia`.
- [x] 3.2 Agent (`crates/provider-java`), unit tests with a session the test controls: the node is invalid while the session is closed or degraded, valid again once the degraded state ends, and the check returns without a call, even when every call of the session fails. Verify that they fail today with `just test-crate platynui-provider-java`.
- [x] 3.3 Extend `live_a_killed_jvm_leaves_no_valid_nodes` (`crates/provider-java/tests/live_fixture.rs:1276`) so that it also asserts that the agent backend's application node reports invalid after the kill, and add a separate live test, `live_a_killed_jvm_leaves_no_valid_application_node_on_the_bridge`, that asserts the same for the JAB backend with the agent disabled. Both are `#[ignore]` and runs in `just test-acceptance-windows`. Verify on Windows that it fails today.

## 4. Application nodes

- [x] 4.1 `ApplicationNode` records its identity in `build` (so `new` and `orphan` both do), and `is_valid` answers from it (design decisions 2 and 4). Verify that 3.1 passes.
- [x] 4.2 `JabAppNode` records its identity in `build`, and `is_valid` answers from it. Verify that the JAB part of 3.3 passes on Windows. Outcome (2026-09-28): `live_a_killed_jvm_leaves_no_valid_application_node_on_the_bridge` passes; before the change it failed at the application node's validity after the kill.
- [x] 4.3 `AgentSession` gains `is_closed()`. `AgentAppNode` records its identity in `new`, and `is_valid` is false while its session is closed or degraded, and otherwise answers from the identity (design decision 5). Verify that 3.2 and the agent part of 3.3 pass. Outcome (2026-09-28): the four tests of 3.2 pass, and `live_a_killed_jvm_leaves_no_valid_nodes` passes in 1 s; before the change it failed after its 30 s deadline.
- [x] 4.4 Rebuild the extension (`just build-native`), and confirm by hand on Windows: `Runtime().evaluate('/app:Application')` returns nodes that report valid, and after a spawned application exits, the node that was taken for it reports `is_valid() == False` and `bool(node) == False`.

  Outcome (2026-09-28):
  - Checked against the repository's egui test app only, through `/app:Application[@ProcessId=<pid>]`. The maintainer's rule keeps applications that ship with Windows out of tests, so the check did not assert on every application node of the desktop.
  - While the app ran, its node `uia://app/<pid>` reported `is_valid() == True` and `bool(node) == True`.
  - After it exited, both were `False`, and the check took 0.07 ms.

## 5. Failing tests first — `PlatynUI.core`

- [x] 5.1 pytest in `tests/PlatynUI/`, with a stub factory that counts `discard_snapshot` and a fake runtime installed with `runtime.override` that counts `clear_cache` (design decision 6). Cover the `ui-context-lookup` scenarios:
  - an element that appears on the third attempt is found, and the snapshot was discarded before each retry;
  - a condition such as "enabled" that fails at first is retried after the snapshot was discarded;
  - `exists()` that gives up at its timeout discards the snapshot, and a later `exists()` finds the element that appeared meanwhile;
  - `get_one`, `get_all` and `iter_all` that find nothing discard the snapshot and report as today;
  - two successful lookups discard nothing;
  - `RuntimeAdapterFactory.find_one` discards when it finds nothing and not when it finds something, and so does `find_all`.

  The existing stubs (`tests/PlatynUI/test_context.py:89-124`) keep working unchanged. Verify that the new cases fail today with `just test-python`.

## 6. `PlatynUI.core`

- [x] 6.1 Add `AdapterFactory.discard_snapshot()` (a no-op that is not abstract) and implement it in `RuntimeAdapterFactory` with `runtime.current.clear_cache()`. `find_one` and `find_all` discard the snapshot when they find nothing. Verify the factory cases of 5.1.
- [x] 6.2 `ContextBase.ensure_that` passes a `failed_func` that discards the snapshot and then invalidates the context, and discards once more when `ensure_that` returns `False` or raises. Verify that 5.1 passes with `just test-python`, and that `just mypy` is clean. Outcome (2026-09-28): the 9 cases of 5.1 pass (8 of them failed before), `just test-python` passes 885 of 885, and `mypy` and `ruff` are clean.

## 7. Acceptance tests (Windows lane, real providers)

Follow the `robot-test-style` skill. Each test launches its own instance, so that the suite's shared instance keeps running.

- [x] 7.1 An egui suite, `tests/acceptance/egui/app_root_after_exit.robot`, without a platform tag:
  - launch a test-app instance and pin its application node with `Set Root    /app:Application[@ProcessId=${pid}]    scope=LOCAL`;
  - end the process;
  - under a short `LOCAL` query timeout, a relative target fails with `RootNotFoundError`, naming the root;
  - `Wait Until Gone` on the application node, captured before the exit, returns.

  On Windows this exercises UI Automation's `ApplicationNode`. On the Linux lanes it exercises AT-SPI's application accessible, which already behaves this way. Verify that it fails today on the Windows lane, with `ElementNotFoundError` after the timeout.

  Outcome (2026-09-28): against the native build of `0802be7`, where no application node overrides `is_valid`, the pinned root fails with `ElementNotFoundError` after the 2 s timeout instead of `RootNotFoundError`, and `Wait Until Gone` fails with `ElementStillPresentError` after 10 s, `uia://app/<pid>` still valid. With the change both pass (9.2).
- [x] 7.2 The same two checks for Swing, served by JAB (`tests/acceptance/swing/`, based on `testapp.resource`) and by the agent (based on `testapp_agent.resource`). The agent's root, and the suite root of `testapp_agent.resource`, name `[@Technology="JavaAgent"]`, so that a degraded session cannot move them to the JAB node (design, Risks). Verify that they fail today on the Windows lane.

  Outcome (2026-09-28): against the native build of `0802be7`, all four fail as 7.1 does, for `jab://app/<pid>` and `agent/app/<pid>`. The agent's pinned root stayed on its node although the session had already turned degraded after the kill. That tree lacks the keyword `Launch Swing Agent Test App`, so the agent suite ran there with the current Swing resources; the product code was the old one. With the change all four pass (9.2).
- [x] 7.3 Rewrite the docstring of `tests/PlatynUI/test_baremetal_root_reuse.py:13-17`: the real proof that a dying root is looked up again is now 7.1 and 7.2, not `window.robot`. Verify by reading.

## 8. Documentation

- [x] 8.1 BareMetal's library introduction (`src/PlatynUI/BareMetal/__init__.py:514-517`): replace "never a stored snapshot" with a short, user-facing description. A keyword may reuse what an earlier keyword read, and the library reads the UI again after a lookup that found nothing, before `Query`, and in every wait. So a suite that expects a changed UI waits for it. Verify with the rendered libdoc of `PlatynUI.BareMetal`.
- [x] 8.2 `crates/core/src/ui/node.rs:63-78` (`UiNode::is_valid`): name the application nodes as nodes with a real lifetime. `dev-docs/architecture.md` §9.3: add that `PlatynUI.core` follows the snapshot protocol too. `dev-docs/python-library-design.md:6049-6051`: the note becomes the implemented behavior. `dev-docs/java-toolkits.md`: the JAB-to-agent handover limit of the design's risks, and how a root pinned by pid alone reaches it through an agent degradation. Verify by reading.

## 9. Verification

- [x] 9.1 Run `just check`, `just test` and `just test-python`, then `just build-native`. Verify that everything is green. Outcome (2026-09-28, Windows): `just check` is clean, `just test` passes 2,400 of 2,400, and `just test-python` 885 of 885; `just build-native` then reinstalled the real build. After the review fixes, run again: `just check` is clean, `just test` passes 2,402 of 2,402, `just test-python` 885 of 885, `robotcode analyze code` finds nothing in the three new suites and `testapp_agent.resource`, and `just build-native` reinstalled the real build.
- [x] 9.2 On Windows, run `just install-provider-java`, then `just test-acceptance-windows`. Verify that everything is green, including 3.3, 7.1 and 7.2. Then verify that `uv run --no-sync robotcode results log --level WARN --execution-messages` shows no PlatynUI warning. Record the outcome in this task.

  Outcome (2026-09-28):
  - The first two lane runs stopped in the live step: `live_a_killed_jvm_leaves_no_valid_application_node_on_the_bridge` waited out its 20 s discovery deadline. The cause lies outside this change. `.config/nextest.toml` serialized only the `provider-java` live binary, and the `java-agent` live tests, added to the same lane step on 2026-07-26, ran in parallel with it. With the bridge enabled machine-wide, their JVMs load it as well, and a JAB test that overlaps their start-ups never sees its own fixture. The existing `live_fixture_contract_and_interaction` fails the same way under that overlap (3 of 3). The new test merely sorts first in the serialized group.
  - Fix, with the maintainer's consent: `platynui-java-agent::live_fixture` joins the `java-live` group. Two runs of the live step then passed 32 of 32; the step takes 65 s instead of about 50 s.
  - Third lane run: green. The live step passed 32 of 32, and Robot Framework passed 125 of 125, the six tests of 7.1 and 7.2 included. `output.xml` holds no WARN or ERROR message.
- [x] 9.3 On a Linux host, run `just test-crate platynui-process`, `just headless=true test-acceptance-x11` (7.1 against AT-SPI) and `just cross-target-checks`. Verify that everything is green. Record the outcome in this task.

  Outcome (2026-09-29), from the maintainer's Linux gate for `application-process-attributes` (its task 7.2, whose step 8 names this task, recorded in `4f0e771`, now under `archive/2026-09-29-application-process-attributes/`). It ran on a clean worktree of `009595b`, which holds all of this change:
  - `just test-crate platynui-process` passes 29 of 29.
  - `just headless=true test-acceptance-x11` passes 93 of 93, and the lane logged no WARN or ERROR. The `real-x11` profile selects `tests/acceptance/egui/app_root_after_exit.robot`, which is tagged `acceptance real` and has no platform tag, so 7.1 ran against AT-SPI.
  - `just cross-target-checks` is clean.

## 10. Commit (only when the user asks)

- [x] 10.1 Commit in reviewable steps, each lint-clean on its own: the process crate; the application nodes (one commit per provider or one for all three); `PlatynUI.core`; the acceptance tests and documentation. Subjects ≤ 72 characters. The commit for the application nodes names the behavior change in its body: a query under an application root whose process has ended fails with `RootNotFoundError`.

  Outcome (2026-09-29): the change was committed on 2026-09-28, at the maintainer's request:
  - `8f6fc02` holds the process crate.
  - `50669a9` puts the java-agent live tests into the JAB group (9.2).
  - `94f5f97` holds the application nodes of all three providers; its body names the `RootNotFoundError` behavior change.
  - `0c2d805` holds `PlatynUI.core`.
  - `23ea907` holds the acceptance tests and `aebf956` the docs.
  - `1f09f50` records the run notes.

  All subjects stay within 72 characters. Each step was checked on its own in a temporary worktree:
  - `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean at `8f6fc02` and at `94f5f97`, the two that change Rust.
  - `ruff check` and `mypy` are clean at `0c2d805`, `23ea907` and `aebf956`, the three that change Python.
