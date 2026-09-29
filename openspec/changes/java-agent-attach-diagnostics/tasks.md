# Tasks

Tests come first in each group. Where they run:

- `crates/java-agent` is portable: `just test-crate platynui-java-agent` runs its unit tests on any OS.
- `crates/provider-java` is Windows-gated as a whole (`crates/provider-java/src/lib.rs:28-35`). Its unit tests run with `just test-crate platynui-provider-java` on a Windows host only; on Linux, `just clippy-windows` compiles them.
- The extension's pytest runs on any OS against the mock build (`just test-python`).

No Robot Framework suite is added: no keyword changes, and the behavior lives inside the provider. The Windows lane's agent suites and its warning check show that healthy runs stay as they are (9.2). The Java Access Bridge exists only on Windows; the refusal of the agent itself is proven on Linux and on Windows by the live check of 2.4.

## 1. Before the change

- [ ] 1.1 On a Windows host with a real build (`just build-native`, `just install-provider-java`), record the state that design decision 1 changes. Start the Swing fixture (`just run-test-app-swing`). In a shell where `VIRTUAL_ENV` is not set and `PLATYNUI_LOG_LEVEL=debug` is, run `.venv\Scripts\python.exe` with a short script: `logging.basicConfig(level=logging.DEBUG)`, `r = platynui_native.Runtime()`, and print `r.evaluate('count(/*[@Technology="JavaAgent"])')`. Expected before the change: `0`, and the debug record "no agent JAR available; automatic attachment is off for this session" with the install remedy, although the package is installed. Record the outcome here, then close the fixture.

## 2. Tests first — `java-agent`

- [ ] 2.1 In `crates/java-agent/src/discovery.rs`, unit tests for the answer parser of design decision 3, a pure function of the exit status, the standard output and the error output:
  - a `java` answer with `agent_jar` and `version` is *found*;
  - `errors.java` is a *broken package* that carries the exception's type and text; use provider-java's own `FileNotFoundError` message;
  - a `java` answer without a usable `agent_jar` or `version` is a *broken package*;
  - no `java` entry is *not installed*, also when another entry point failed;
  - a top-level `error` is an *interpreter failed* that carries it;
  - a non-zero exit is an *interpreter failed* that carries the last line of the error output;
  - output that is not JSON is an *interpreter failed* that carries its start, shortened.

  Verify with `just test-crate platynui-java-agent` that they fail to compile before the change.
- [ ] 2.2 Unit tests for the choice of interpreter (design decision 1), as a function of the named interpreter, the running executable's path and `VIRTUAL_ENV`:
  - a named interpreter wins over a `pyvenv.cfg` layout and over `VIRTUAL_ENV`;
  - without one, today's inference holds; extend `an_interpreter_is_only_accepted_from_a_real_environment_root` (`discovery.rs:327-339`);
  - with none of them, the outcome is *not installed*, and its details say that no Python environment was found and name `providers.java.agent.jar` and `PLATYNUI_JAVA_AGENT_JAR`.

  Also the guards of the hand-over, as a pure function of the executable path and the frozen flag that the extension will call: an empty path, a frozen application, and a file name that does not start with `python` name nothing; `python`, `python3.12`, `python.exe` and `pythonw.exe` pass. Verify that they fail to compile before the change.
- [ ] 2.3 Unit tests for the resolution order and its causes (design decision 4), on a resolution core that takes the configured path, the variable's value and the lookup as inputs:
  - a configured path that is not a file is *not a file*, names `providers.java.agent.jar` and the path, and the lookup is not consulted; update `an_explicit_path_wins_and_must_exist` (`:284-300`);
  - the same for the variable, naming `PLATYNUI_JAVA_AGENT_JAR`;
  - an installed package of another version is a *version mismatch* that names both versions and reinstalling the extra;
  - a package whose JAR is not a file is a *broken package*;
  - a broken package and a failed interpreter carry their text, and never name `robotframework-platynui[java]`;
  - *not installed* keeps its remedy; extend `the_missing_package_diagnostic_names_the_remedy` (`:302-315`) to assert the cause.

  Verify that they fail to compile before the change.
- [ ] 2.4 In `crates/java-agent/tests/live_fixture.rs`, an ignored live check that a JVM which disallows dynamic agent loading refuses the agent — the real-JVM evidence behind the warning of `java-provider`'s *A JVM that refuses the agent is named once, with the remedy*:
  - `FixtureJvm` takes the launcher as an option; the default stays `swing_java_launcher()`;
  - the test launches the fixture on the `java21` launcher of `apps/test-app-swing/build/java-launchers.properties`, which `just build-test-app-swing` writes and the recipe builds first, with `-XX:-EnableDynamicAgentLoading`. A missing `java21` entry fails the test with a message naming the file;
  - `attach::load_agent` returns `AgentRefused`, whose details contain `EnableDynamicAgentLoading`.

  The classification exists, so this passes today. Verify with `just test-java-agent-live` on Linux; the Windows run is part of 9.2.
- [ ] 2.5 In `crates/java-agent/tests/delivery.rs` (ignored):
  - both existing tests follow the lookup's outcome instead of an `Option`;
  - a new test installs the wheel, deletes the installed `platynui-agent.jar`, and asserts that the query reports a *broken package*. Its text carries provider-java's `FileNotFoundError` message and "reinstall platynui-provider-java", and not `robotframework-platynui[java]` (`java-agent`: *A broken installation reports its own cause*).

  Verify with `cargo check -p platynui-java-agent --tests`. The run is 9.4.

## 3. Tests first — `provider-java` (Windows host)

- [ ] 3.1 In `crates/provider-java/src/agent/backend.rs`, unit tests with log capture (`logged` and `lines_at`, `:609-639`) for the attach pass of design decisions 5 and 6. The pass takes its resolution and its attach call as parameters. The tests inject both; the injected attach only counts its calls. Scenarios of `java-provider`:
  - *An explicit JAR path that is not a file warns once*: over three passes, exactly one WARN naming the path and "not served through the agent", DEBUG for the second and third pass, the attach never called, and no attempt recorded for the pid;
  - *Each runtime says it once*: a second backend with the same failure warns once too; one backend that meets two different mistakes warns once for each;
  - *A broken installation warns once, with its own cause*: the WARN carries the injected text, and no record contains `robotframework-platynui[java]`;
  - *An absent package produces no warning*: over several passes, no WARN, and exactly one DEBUG naming the install;
  - *A JAR that resolves later still gets every attempt*: four failing passes, then one that succeeds; the attach is called for the pid, and one DEBUG says that the JAR is available again;
  - *Nothing is reported while automatic attachment is off*: with `auto_attach = false`, and with the backend disabled, the resolution is never called and nothing is logged.

  No record may contain "Access Bridge". Verify with `just clippy-windows` that they fail to compile before the change.
- [ ] 3.2 Unit tests for the attach outcomes of design decision 7, with the attach injected:
  - *A JVM that refuses the agent is named once, with the remedy*: `AgentRefused` on all three attempts of one pid gives exactly one WARN, which names the pid and contains `EnableDynamicAgentLoading`, `JAVA_TOOL_OPTIONS`, `-javaagent` and `auto_attach`, and two DEBUG records; a second pid that refuses warns on its own;
  - *An attach that fails for other reasons stays at debug*: `AttachFailed`, `NotAJvm` and `ProcessUnavailable` give no WARN.

  Verify as in 3.1.
- [ ] 3.3 Tests through the production path, `consider_attaching`, without a JVM:
  - with `providers.java.agent.jar` naming a temporary file, the test's own pid is attempted: `load_agent` stops at `NotAJvm` before any attach (`crates/java-agent/src/attach/mod.rs:75-77`), there is no WARN, and one attempt is recorded;
  - a pid that no process has (`u32::MAX`) gives `ProcessUnavailable` and no WARN;
  - with a path that does not exist, there is one WARN and no attempt.

  Verify as in 3.1.
- [ ] 3.4 Replace `an_invalid_timeout_falls_back_to_the_default` (`backend.rs:602-607`) with a log-capturing test (`diagnostic-logging`: *A Java agent call timeout of zero*): a `call_timeout_ms` of `0`, and one of `-1`, each give exactly one WARN naming `providers.java`, `key=agent.call_timeout_ms` and the value, and the default applies; `750` and an absent key give none. Verify as in 3.1.

## 4. Tests first — the Python extension

- [ ] 4.1 Add `packages/native/tests/test_java_agent_discovery.py`.
  - **Setup.** Each case runs in a subprocess, as `test_test_build_warning.py` does, with `VIRTUAL_ENV` and `PLATYNUI_JAVA_AGENT_JAR` removed from its environment. A stand-in distribution is written to a temporary directory placed first on `PYTHONPATH`: a `platynui_provider_java-<version>.dist-info` with `METADATA` and an `entry_points.txt` that registers `java` in `platynui.providers`, pointing to a module of its own. It shadows an installed package of the same name (design decision 9). The mock-build hooks of 7.3 are reached through `getattr`, as `test_native_logging.py:26` does.
  - **Cases**, from the `java-agent` scenarios:
    - *A test run finds the artifact of the interpreter it runs in*: the stand-in returns a temporary JAR and the version the build expects, and the resolution returns that JAR;
    - *A broken installation reports its own cause*: the stand-in raises `FileNotFoundError('… reinstall platynui-provider-java')`; the error carries `FileNotFoundError` and the message, and not `robotframework-platynui[java]`;
    - *An installed package of another version is named*: the error names both versions;
    - *Importing the extension resolves nothing*: the stand-in's module writes a marker file when it is imported; after the import of the extension and `Runtime.new_with_mock()`, the named interpreter equals `sys.executable`, and the marker does not exist;
    - *A host that is not a Python interpreter is not taken for one*: with `sys.frozen = True` set before the import, and in a second subprocess with `sys.executable` assigned the path of a file named `app` (`app.exe` on Windows), no interpreter is named.

  Verify with `uv run --no-sync pytest packages/native/tests/test_java_agent_discovery.py` that they fail before the change, because the hooks are missing.

## 5. `java-agent`

- [ ] 5.1 Give `AgentError::JarUnavailable` its `cause` (design decision 4, `crates/java-agent/src/error.rs:63-70`), and set it at every producer: `discovery.rs:97-116`, `:124-133` and `:135-143`, and the attach's own checks at `attach/mod.rs:63-68` and `:81-84`. Verify with `just test-crate platynui-java-agent` that 2.3 compiles.
- [ ] 5.2 Rewrite the query and how it is read (design decision 3):
  - the snippet (`discovery.rs:173-186`) reports errors per entry point and for `importlib.metadata` itself, and guards the serialization of each answer;
  - `query_interpreter` (`:227-242`) returns the lookup's outcome through the parser of 2.1;
  - `installed_package` (`:145-167`) caches that outcome.

  Verify that 2.1 passes.
- [ ] 5.3 Replace `set_in_process_resolver` and its resolver type (`discovery.rs:66-81`) with the hand-over of design decision 1: the guard function of 2.2, a set-once store, the choice of interpreter with the named one first and authoritative, and one debug record naming the interpreter and how it was chosen. Verify that 2.2 passes, and that `git grep -n in_process_resolver` finds nothing.
- [ ] 5.4 Split `resolve_agent_jar` (`:96-117`) into the resolution core of 2.3 and a thin wrapper that reads the variable and the cached lookup, with a remedy text per cause; only *not installed* names the install. Verify that 2.3 passes and that 2.5 compiles.
- [ ] 5.5 Rewrite the module docs (`discovery.rs:1-31`): one query, asked of one interpreter, which is found in one of two ways — named by the Python extension, or inferred by a standalone binary — and why no Python code runs in the runtime's own process. Verify with `just clippy` and `just doc`.

## 6. `provider-java`

- [ ] 6.1 Rework `attach_to_agentless` (`crates/provider-java/src/agent/backend.rs:257-299`) along design decisions 5 and 6:
  - the resolution and the attach are parameters; `consider_attaching` passes `resolve_agent_jar` and `attach::load_agent`;
  - the eligible pids are collected without charging them;
  - the JAR is resolved, and the outcome reported through a `Transitions` latch in the backend: a warning for a mistake, debug for *not installed*, and one debug record per episode that the next success ends;
  - attempts are charged under the attempts lock, right before each attach.

  Verify that 3.1 and 3.3 pass on a Windows host, and that `just clippy-windows` is clean on Linux.
- [ ] 6.2 Turn the attempt record (`:96-99`) into a per-pid record of the attempts and whether a refusal was reported, and report the attach errors along design decision 7. Update the doc comments of `MAX_ATTACH_ATTEMPTS` (`:58-65`) and of `attach_to_agentless` (`:244-256`), which describe the attempts. Verify that 3.2 passes.
- [ ] 6.3 Warn in `from_config` (`:127-131`) for a `call_timeout_ms` of zero or less (design decision 8). Verify that 3.4 passes.

## 7. The Python extension

- [ ] 7.1 Add `platynui-java-agent` to `[dependencies]` in `packages/native/Cargo.toml`, for every platform (design decision 2). Verify with `just clippy`.
- [ ] 7.2 Add a module next to `packages/native/src/log_bridge.rs` that names the interpreter at import:
  - it reads `sys.executable` and `sys.frozen`, applies the guard function of 5.3, and hands the path to `java-agent`;
  - it records a refusing guard, or a failure to read, at debug, and never fails the import;
  - `_native` calls it after `log_bridge::install` (`packages/native/src/lib.rs:25-27`).

  Verify with `just clippy`.
- [ ] 7.3 Add the two mock-build hooks of design decision 2 to that module, registered like `_emit_log_for_tests` (`log_bridge.rs:459-466`). Verify with `just clippy`.
- [ ] 7.4 Rebuild with `just build-native-mock`, then run `just test-python`. Verify that 4.1 passes and that no other test changed its result.

## 8. Documentation

- [ ] 8.1 Update the docs that describe the discovery or the attach:
  - `packages/provider-java/README.md:27-31` (*Finding the JAR*): discovery asks the interpreter of the test run, or the environment's interpreter for a standalone binary;
  - the docstring of `provider_info` (`packages/provider-java/src/platynui_provider_java/__init__.py:42-53`) and the comment in `packages/provider-java/pyproject.toml:27-28`, which speak of "the in-process lookup";
  - `dev-docs/java-toolkits.md`, *Getting the agent* (`:281-298`): where the package is looked for; what is reported at which level — an absent package at debug, mistakes as warnings once per runtime, a refusing JVM once with its remedy; and that a JAR that cannot be resolved costs no attempt. *What has to be true for a target to be served* (`:299-327`) gains the refusal warning next to the JEP 451 remedy;
  - `dev-docs/platform-windows.md`, the automatic-attachment bullet (`:97`): the attempts and the reports;
  - `dev-docs/python-bindings.md`: what the extension does at import besides installing the log bridge — it names its interpreter for the agent's discovery, reads no package metadata, and discovery never calls back into Python;
  - `dev-docs/logging.md`: in §9 (`:321-365`), a value of the right type that cannot be used, with the agent's call timeout as the example; in §2 (`:75-83`), the refusal as a second thing the Rust side reports from outside the target.

  Verify by reading, and with `just check`.
- [ ] 8.2 Record the outcome in the review ledger, `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`, as the triage recorded its own (`009595bc`): the status lines at `:526`, `:531` and `:715` become "implemented by `java-agent-attach-diagnostics`"; `:570` and `:793` become revived and implemented, with the reason that their premise was false; `:600` becomes partly implemented, with the agent's timeout done and the JAB half open. Verify by reading.

## 9. Verification

- [ ] 9.1 On Linux, run `just check`, `just test` (it runs the `java-agent` tests of section 2), `just test-python`, `just clippy-windows`, and `just test-java-agent-live` (2.4 on the Unix attach leg). Verify that everything is green.
- [ ] 9.2 On a Windows host, with the maintainer's go-ahead: run `just test` (it runs the `provider-java` tests of section 3), `just build-native`, `just install-provider-java` and `just test-java-agent-live`, then `just test-acceptance-windows` and `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - everything is green, including the agent suites (`tests/acceptance/swing/*agent*`), whose JAR now comes from the interpreter of the run itself;
  - there is no WARN or ERROR from PlatynUI in the run.

  Record the outcome here.
- [ ] 9.3 On the same Windows host, by hand, against the real build, each with `logging.basicConfig()` and several evaluations of `/*`. Record the outcomes here:
  - repeat 1.1: the fixture is now served through the agent although `VIRTUAL_ENV` is not set, and `sys.executable` names `.venv\Scripts\python.exe`. Where a conda environment with the package is at hand, check the same there; where a runner uses `pythonw.exe`, check it with that interpreter (design, Risks);
  - the fixture started on the `java21` launcher with `-XX:-EnableDynamicAgentLoading`: exactly one refusal warning, naming its pid, the refusal and the remedies, and not the Access Bridge. The bridge may add its own enablement warning for that JVM; it is not counted here;
  - `PLATYNUI_JAVA_AGENT_JAR` set to a path that does not exist: exactly one warning, naming the variable and the path;
  - `Runtime({'providers': {'java': {'agent': {'call_timeout_ms': 0}}}})`: one warning naming `agent.call_timeout_ms` and `0`.
- [ ] 9.4 Run `just test-provider-java-delivery`. It is a heavy recipe, and warranted here because this change rewrites the query it exercises. Verify that 2.5 and the two existing delivery tests pass.

## 10. Commit (only when the user asks)

- [ ] 10.1 Commit in reviewable steps, each lint-clean and green on its own:
  - the `java-agent` discovery and cause, with the extension's hand-over and hooks, and the tests of sections 2 and 4;
  - the `provider-java` reports, attempts and timeout, with the tests of section 3;
  - the docs and the review ledger.

  Conventional Commits, subjects at most 72 characters, no `!`. The bodies list the behavior changes of the proposal for the release notes. Do not push.
