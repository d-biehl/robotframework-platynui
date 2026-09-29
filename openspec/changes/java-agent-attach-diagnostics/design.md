# Design

## Context

See proposal.md for the motivation and the specs for the required behavior.

Line numbers are those of commit `7180d20b` (2026-09-29). *Verified* marks what was read in the code. *Inferred* marks conclusions drawn from the code that were not run. *External* marks facts about Python or the JDK that the repository cannot show.

**How the agent JAR is resolved today (verified).**

- **The order.** `resolve_agent_jar(configured)` (`crates/java-agent/src/discovery.rs:96-117`) tries:
  1. the configured path, `providers.java.agent.jar` (`:97-99`);
  2. the environment variable `PLATYNUI_JAVA_AGENT_JAR` (`:100-102`);
  3. the installed package (`:104`), whose version must equal the client's (`:106-115`) and whose JAR must be a file (`:116`).

  A configured or environment path that is not a file is an error. The package is never used in its place (`require_file`, `:135-143`).
- **One error for everything.** Every failure is `AgentError::JarUnavailable { details, path }` (`error.rs:63-70`). Only the text of `details` tells the causes apart.
- **The cache.** `installed_package()` caches one `Option<AgentPackage>` per process (`:145-152`). A lookup that found nothing, for whatever reason, is cached as `None` and becomes the install remedy (`missing_package_error`, `:124-133`).
- **The two transports.** `discover_package()` asks the in-process resolver when one is installed, and the environment's interpreter otherwise (`:154-167`).
- **The query.** `QUERY_SNIPPET` (`:173-186`) skips an entry point that raises (`:181-182`) and prints `{}` when `importlib.metadata` itself fails (`:184-185`). `query_interpreter` (`:227-242`) returns `None` for:
  - a failure to start the interpreter (`:231`);
  - a non-zero exit (`:232-235`), the only case with a record, at debug;
  - output that is not JSON (`:236`);
  - a missing or malformed `java` entry (`:237-240`).

  It is public, and the ignored delivery test calls it (`crates/java-agent/tests/delivery.rs:116`, `:154`).
- **Which interpreter.** `environment_interpreter()` (`:251-260`) looks for `python.exe` (Windows) or `python3`/`python` (elsewhere) in the running executable's own directory. It accepts one only when the parent directory holds a `pyvenv.cfg` (`:272-278`), and otherwise takes the one under `VIRTUAL_ENV`.

**The in-process path never ran (verified).**

- `set_in_process_resolver` (`discovery.rs:79-81`), with the resolver type `fn() -> Option<AgentPackage>` (`:66`), has no caller. `git log -S set_in_process_resolver` finds only `09d97a63`, the commit that introduced it, and no file of the repository calls it.
- `packages/native` depends on `platynui-provider-java` on Windows only (`packages/native/Cargo.toml:43-46`) and on no Java crate directly. Its module init registers the types and installs the log bridge, nothing else (`packages/native/src/lib.rs:17-30`).
- *Inferred:* in a Robot Framework run, discovery therefore takes the standalone path, with the Python interpreter in the role of the binary: `std::env::current_exe()` is the running interpreter.
  - A virtual environment passes only when its interpreter is a real file in its `bin` or `Scripts` directory, as with `--copies` on Linux, because `pyvenv.cfg` sits in the environment's root, above that directory.
  - A conda environment and a system-wide installation have no `pyvenv.cfg`, and fail unless `VIRTUAL_ENV` is set.
  - On Linux, `/proc/self/exe` resolves a virtual environment's symlinked `python` to the base interpreter, whose directory has no `pyvenv.cfg`.
  - On Windows, a virtual environment's `Scripts\python.exe` — CPython's redirector, or uv's trampoline — runs the base interpreter as a separate process, so `current_exe()` names the base interpreter. The justfile documents the trampoline (`justfile:21-26`).
  - `uv run` sets `VIRTUAL_ENV`, which is why the lanes find the package.

**Who reports what (verified).**

- **The attach pass.** `attach_to_agentless` (`crates/provider-java/src/agent/backend.rs:257-299`):
  1. charges one attempt per pid while it collects the candidates, before anything else (`:273-277`);
  2. resolves the JAR (`:283`) and records every error at debug, as "no agent JAR available; automatic attachment is off for this session" (`:286`);
  3. records every attach error at debug (`:294`).

  `MAX_ATTACH_ATTEMPTS` is 3 (`:65`). The attempt record (`:99`) is keyed by pid and never pruned.
- **The existing reports above debug.** A mismatched agent already in a JVM is warned about through a `Transitions<u32>` latch in the backend (`:103`, `:204-217`). An agent that was injected but published no handshake is warned about (`:321-334`).
- **The transport's classification.** `load_agent` (`crates/java-agent/src/attach/mod.rs:62-94`) returns, before any attach:
  - `JarUnavailable` for a JAR that is not a file or cannot be canonicalized (`:63-68`, `:81-84`);
  - `ProcessUnavailable` for a process that is not running (`:69-71`);
  - `NotAJvm` for a process known not to run a JVM (`:75-77`).

  From the JVM's reply, it returns `AgentRefused` for a non-zero `Agent_OnAttach` result (`:162-164`) and for the three refusal markers, JEP 451's among them (`:177-185`). Everything else is `AttachFailed`.
- **The candidates.** The router calls `consider_attaching` with the pids of the Java windows its sweep saw (`crates/provider-java/src/provider.rs:260-277`).
- **The timeout.** `from_config` (`backend.rs:118-148`) reports an unknown key and a value of the wrong type through the provider's helpers (`provider.rs:373-392`). It then drops a zero or negative value silently (`backend.rs:127-131`), and `an_invalid_timeout_falls_back_to_the_default` pins that (`:602-607`).
- **The tests' tools.** The backend's tests already capture records by level (`logged` and `lines_at`, `backend.rs:609-639`). The provider crate is Windows-gated as a whole (`crates/provider-java/src/lib.rs:28-35`), so its unit tests run on a Windows host only.

**Why no Python may run inside a runtime call (verified; the deadlock is inferred).**

- `PyRuntime::evaluate` holds the interpreter (the GIL) for the whole call (`packages/native/src/runtime.rs:883-901`) and takes the runtime's `Mutex` through `runtime()` (`:796-798`).
- No code in `packages/native/src` detaches from the interpreter: there is no `detach` and no `allow_threads`. The log bridge attaches again only after the runtime lock has been released (`runtime.rs:752-757`, `log_bridge.rs:307-309`).
- The JAR is resolved inside such a call: the enumeration reaches `consider_attaching`, then `resolve_agent_jar`, then `installed_package()`.
- *Inferred:* a resolver that ran Python bytecode there could lose the GIL, at the interpreter's switch interval or during its file reads. Another Python thread could then call any `Runtime` method and block on the runtime lock while it holds the GIL, and the resolver could never continue.
- The same shape is forbidden for log records by the native-logging spec (*Logging never blocks or deadlocks native code*), and `dev-docs/python-bindings.md` (*Logging*) explains why native code never waits for the interpreter.

**What `sys.executable` names (external).**

- It is the path of the running interpreter.
- In a Windows virtual environment it names the environment's `Scripts\python.exe`, not the base interpreter, whether that file is CPython's redirector or uv's trampoline. `sys._base_executable` names the base. Starting `sys.executable` again therefore enters the same environment.
- It is empty, or `None`, when Python cannot tell.
- In a frozen application (`sys.frozen`, set by PyInstaller and similar tools) it is the application itself. An embedding host reports its own executable.
- Confirmed on Windows by task 9.3.

**The review entries (verified).** Follow-up A4 of `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`:

- `:522-526`: the JAR errors at debug (A4);
- `:527-531`: the refusal at debug (ride-along A4);
- `:711-715`: the swallowed entry-point error (ride-along A4);
- `:599-603`: the timeout, whose agent half rides along;
- `:566-570` and `:790-793`: the query's start and JSON failures, dropped because "in Robot Framework the in-process resolver answers first" — which was never true.

## Goals / Non-Goals

**Goals:**

- A test run finds the package of the interpreter it runs in, whatever kind of environment that is, without running Python code inside a runtime call.
- Every reason why automatic attachment cannot inject the agent is visible at the level its cause deserves, once, in words that are true on every platform.
- A JAR that cannot be resolved does not cost a JVM its attempts.

**Non-Goals:**

- The standalone binaries (Inspector, CLI) in a conda environment or a system-wide installation. Their inference keeps the `pyvenv.cfg` rule (Open Questions).
- A deadline for the interpreter query (Open Questions).
- How often a refusing JVM is attacked: three attempts per JVM and runtime stay, a refusal included (Open Questions).
- The Access Bridge's own `call_timeout_ms` and the other JAB entries of the review.
- Reporting an unserved Java window on Linux. That is the router's enablement diagnostic, which `java-provider-linux` defines.

## Decisions

### 1. The extension names its interpreter at import, and discovery asks it in a child process

- **At import.** The module init of `_native` reads `sys.executable` and hands the path to `platynui_java_agent::discovery`, through a new `set_host_interpreter`. That replaces `set_in_process_resolver` and its resolver type, which have no caller.
  - It reads one attribute of `sys`, once. Nothing is imported and nothing is resolved, so *Quiescence when inactive* holds.
  - It runs after `log_bridge::install`, so its debug records reach Python.
  - A failure while reading is recorded at debug and otherwise ignored: Java discovery must never fail the import.
- **The guards.** No interpreter is named when:
  - `sys.executable` is empty or `None`;
  - `sys.frozen` is set;
  - the file name does not start with `python`, compared without case: `python`, `python3`, `python3.12`, `python.exe` and `pythonw.exe` pass.

  An embedding host or a frozen application reports its own executable, and starting that with `-c` would start the application. A guard that refuses records at debug which rule applied.
- **The lookup.** The first time a JAR is needed, the cached lookup asks the named interpreter. It uses the same query, the same console suppression (`hide_console`, `discovery.rs:192-219`) and the same parsing as the standalone path. Without a named interpreter, it falls back to today's inference.
- **The named interpreter is authoritative.** When it answers "not installed", the inference is not tried as well: it could only find another environment's package.
- **The decision is named** (`dev-docs/logging.md` §6). The debug record of a found package names the interpreter and how it was chosen: the process's own interpreter, the binary's environment, or `VIRTUAL_ENV`.

*Alternatives rejected:*

- **Wire the in-process resolver.** The deadlock above.
- **Resolve the entry point at import and hand over its result.** It gives exact in-process fidelity and needs no child process. But every import, on every platform with the Java provider, would read the metadata of the whole environment and import `platynui_provider_java`, whether or not a JVM is ever met. That happens even before the configuration that may disable the agent is known. It contradicts *Quiescence when inactive* and `discovery.rs:27-31`, and it keeps two transports with two error paths.
- **Resolve in `Runtime.__new__`, before the runtime lock exists.** The same objection: the default configuration enables the agent, so every Windows runtime would read metadata in sessions that never meet a JVM. The binding would also have to know the provider's settings.
- **Hand over `sys.prefix` and derive the interpreter.** It derives what `sys.executable` states, and the layout differs per platform and distribution: conda's `python.exe` sits in the environment's root on Windows, not in `Scripts`.
- **Resolve in Python after the runtime call returns, the way the log bridge delivers.** The pass that meets the JVM could not attach, which breaks "served through the agent in that same enumeration" (`java-provider`). It would also need plumbing from `java-agent` into the extension's call guard.

*Consequence:* a conda environment, a system-wide installation, an interpreter started without activation and a Windows virtual environment's redirector all resolve as an activated environment does. The lanes do not change: under `uv run`, `sys.executable` is the project environment's interpreter.

### 2. The extension depends on `platynui-java-agent` on every platform

- **Where.** `packages/native/Cargo.toml` gains `platynui-java-agent` under `[dependencies]`, not in the Windows section next to `platynui-provider-java` (`:43-46`). The crate is portable and depends on no PlatynUI crate (`crates/java-agent/Cargo.toml`), so the cost is its compile time.
- **Why.**
  - The hand-over is plain data on every platform.
  - `just test-python`, and CI's pytest job, run on Linux (`.github/workflows/ci.yml:190-240`), and they should see it.
  - `java-provider-linux` needs the dependency on Linux anyway.
- **The test hooks.** Two hooks are compiled into the mock build only, like `_emit_log_for_tests` (`log_bridge.rs:419-466`), and reached through `getattr`, as `test_native_logging.py:26` does:
  - one returns the named interpreter, and the version the build expects of the package;
  - one runs the resolution and returns the JAR, or raises with the error's text.

*Alternative rejected:* the dependency on Windows only, next to `platynui-provider-java`. The pytest could then run only on a Windows host, and `java-provider-linux` would have to move it.

### 3. The query reports failures, and the lookup has outcomes

- **The answer.** The query prints `{"providers": {name: answer}, "errors": {name: "Type: message"}}`, plus `"error"` when `importlib.metadata` itself fails. Each entry point is loaded, called, and its answer checked for JSON serializability, inside a guard of its own. A broken third-party entry point in the group therefore still cannot keep ours from being found, which is what the snippet promises (`discovery.rs:171-172`).
- **The outcomes.** The lookup, cached per process as today, distinguishes:
  - *found*;
  - *not installed*: the group has no `java` entry, or no environment could be found to ask; the details say which;
  - *broken package*: `errors.java`, or a `java` answer without a usable `agent_jar` and `version`;
  - *interpreter failed*: it could not be started; it exited with an error, and the last line of its error output is carried; or it printed something that is not an answer, carried shortened.
- **The API.** `query_interpreter` and `installed_package` return that outcome instead of an `Option`, and the delivery test follows.
- **The dropped entry.** This revives the dropped entry at `discovery.rs:231` (review `:566`, `:790`). The premise of its drop, that Robot Framework asks in-process first, was false. And a failing interpreter now lies on the path of every test run that needs the JAR.

### 4. `JarUnavailable` carries its cause

- **The field.** `AgentError::JarUnavailable` gains a `cause`. `details` keeps the text with the remedy, and `path` stays.
- **The causes and their remedies:**
  - *not installed*: install the extra, or set the explicit path. This is the only cause that names the install.
  - *not a file*: the setting, the variable, or the JAR handed to an attach; fix the setting or the variable.
  - *version mismatch*: reinstall the extra, so that the versions match.
  - *broken package*: reinstall `platynui-provider-java`, with the entry point's own text carried. A package whose reported JAR is not a file (`discovery.rs:116`) is broken too.
  - *interpreter failed*: set the explicit path, or check the interpreter, with its own message carried.
- **The consumer** decides the level by the cause, not by matching text (`dev-docs/error-handling.md`, *Testing Guidance*).

*Alternatives rejected:*

- **A separate `AgentError` variant per cause.** The attach's own JAR check and the resolution share the variant and its `path`, and every consumer then matches one variant.
- **Matching on `details`.** Fragile, and the text is written for people.

### 5. The attach pass reports, once per runtime and cause

- **Who.** `java-agent` returns the error. The agent backend swallows it, because it skips the attach, and so it is the one layer that reports it above debug (`dev-docs/logging.md` §4).
- **How often.** A `Transitions` latch in the backend, keyed by the rendered error:
  - a mistake — every cause except *not installed* — warns when its episode starts, and adds a debug record for each further occurrence;
  - *not installed* starts its episode with a debug record that names the install, and continues silently;
  - the next successful resolution ends every open episode with one debug record each (`retain`), saying that the JAR is available again.
- **The warning.** `warn!(error = %error, "agent JAR unavailable; automatic attachment is skipped, so Java applications without an agent are not served through it")`, with `path` where it is known. The field names follow `dev-docs/logging.md` §11.
- **Why per runtime.**
  - It is what the triage asked for: warn once per session and cause.
  - It is how the backend already reports a mismatched agent (`backend.rs:103`).
  - A runtime has one configuration, so its latch sees one source at a time, and its episodes end cleanly.
  - Every Robot Framework suite has a runtime of its own, because `BareMetal` is suite-scoped (`src/PlatynUI/BareMetal/__init__.py:566-567`). Each suite's log therefore says why its Java applications are not served through the agent.
- **When.** The mistake is reported when it matters, not at build. `from_config` does not check the configured path:
  - the file may legitimately appear later;
  - a check at build would warn in sessions that never meet a JVM;
  - the installed package cannot be looked up at build without breaking *Quiescence when inactive*.
- **The wording.** Every record says "not served through the agent". None names the Access Bridge, because on Linux there is none.

*Alternative rejected:* once per process, like the missing bridge DLL (`provider.rs:59-60`, `dev-docs/logging.md` §5). It would give one warning per run for a mistake that concerns the whole process. But runtimes with different settings would end each other's episodes, unless the latch were keyed by the source of the resolution, which `resolve_agent_jar` does not report. And the logs of later suites would not say why.

### 6. Resolve before charging attempts

- **The order.** The pass:
  1. collects the eligible pids without charging them: not 0, no session, no handshake file, and fewer than `MAX_ATTACH_ATTEMPTS` attempts;
  2. returns when there are none;
  3. resolves the JAR, and returns on a failure without charging anything;
  4. charges each pid under the attempts lock and attaches it. A pid whose budget a concurrent pass used up meanwhile is dropped at this point, so concurrent passes cannot exceed the budget.
- **The cost.** While the JAR is unavailable, every pass with a candidate resolves again: one environment read and one file check, or the cached lookup. That is the work of today's first three passes, without the stop after them.

*Alternative rejected:* keep charging, and document it. A JAR that appears later — on a network share, or copied by a setup step — would find the attempts of its JVMs spent.

### 7. A refusal is warned once per JVM and runtime

- **The record.** The per-pid attempt record becomes a small record: the number of attempts, and whether a refusal was reported. The first `AgentRefused` of a pid warns; later ones are debug. Every other attach error stays at debug.
- **The warning.** `warn!(pid, error = %error, "the JVM refused the PlatynUI agent; this application is not served through the agent (a JVM that disallows dynamic agent loading needs -XX:+EnableDynamicAgentLoading, which JAVA_TOOL_OPTIONS can carry, or -javaagent at launch; providers.java.agent.auto_attach = false stops the attempts)")`.
  - The JVM's own text travels in `error`. For JEP 451 it names the flag itself.
  - `JAVA_TOOL_OPTIONS` is named because it is the only channel into a Web Start target (`dev-docs/java-toolkits.md`, *What has to be true for a target to be served*).
- **Why a warning.** Installing the package is the consent to instrument, automatic attachment is on, and the target decided against it. PlatynUI knowingly works with less because of the target application (`dev-docs/logging.md` §3). That is not an event of a healthy session.
- **Why the other failures stay at debug.**
  - `AttachFailed` covers the transient case that the attempts exist for (`backend.rs:58-64`), and a JVM that does not answer. For a JVM it holds, `jab-discovery-containment` reports that once per episode.
  - `NotAJvm` and `ProcessUnavailable` tell the user nothing to act on.

*Alternatives rejected:*

- **Warn when the last `AttachFailed` attempt fails**, as the review proposed (`review-findings.md:528`). The triage rejected it. Its "served by the Access Bridge only" does not hold on Linux, and it would duplicate the held-JVM report.
- **A `Transitions<u32>` latch, like the version mismatch.** Nothing tells the backend that a refusing JVM is gone, because it never has a session with it. The attempt record already bounds the repeats to two debug records.

### 8. The agent's call timeout warns when it is zero or less

- `from_config` warns when `call_timeout_ms` is an integer of zero or less, and the default applies: `warn!(component = "providers.java", key = "agent.call_timeout_ms", value, default_ms = 5000, "setting must be a positive number of milliseconds; the default applies")`.
- A value of the wrong type keeps its existing warning, which already ends in the default (`provider.rs:382-392`). A positive value and an absent key stay silent.
- The JAB backend's identical parse (`crates/provider-java-jab/src/provider.rs:168-171`) is left to the JAB change, so the spec names the agent's key only.

### 9. Where each property is proven

- **`java-agent`, on every OS:**
  - the answer parser, the choice of interpreter, and the resolution order, written as functions that take their inputs — the named interpreter, the executable's layout, `VIRTUAL_ENV`, the variable's value and the lookup — so that no test touches process-wide state;
  - the live refusal, on a JDK 21 fixture;
  - the delivery test, with the real wheel.
- **`provider-java`, on a Windows host:**
  - the attach pass takes its resolution and its attach call as parameters; production passes `resolve_agent_jar` and `attach::load_agent`. Every cause and every attach outcome is then driven without a JVM or a package;
  - the real path, against the test's own process, which is not a JVM: `load_agent` stops at `NotAJvm` before any attach (`attach/mod.rs:69-77`).
- **The extension, on every OS:** pytest in subprocesses, with a stand-in distribution named like the real package, first on `PYTHONPATH`. *External:* `importlib.metadata` keeps the first distribution of each name, so the stand-in wins whether or not the real package is installed in the development environment.
- **By hand, on Windows, against a real build:** an interpreter started without activation, a refusing JDK 21 fixture, a wrong `PLATYNUI_JAVA_AGENT_JAR`, and a timeout of zero.
- **No Robot Framework suite.** No keyword changes, and the property lives inside the provider. The Windows lane's agent suites and its warning check show that healthy runs are unchanged.

## Risks / Trade-offs

- **[The child's module path can differ from the host's]** → A path added at run time (`sys.path.insert`, Robot Framework's `--pythonpath`) is not inherited; `PYTHONPATH` is. The package is found through the installed metadata in the environment's site directories, which both processes see. The explicit setting remains the way around anything else.
- **[A frozen or embedding host]** → No interpreter is named, and the inference usually finds none either. The diagnostic says that no environment was found and names the explicit setting, at debug, like an absent package. A frozen runner that bundles the package needs the explicit setting.
- **[`pythonw.exe` as the host]** → *Inferred:* its standard output reaches the pipe when one is given. Task 9.3 checks it on Windows.
- **[Environments that never had the agent get it now]** → An installed package is consent, so this is the documented behavior, but the tree changes from the bridge's to the agent's. It is listed in the release notes, with the opt-outs: `providers.java.agent.auto_attach = false`, or uninstalling the extra.
- **[One interpreter start per process]** → It is paid in the first pass that meets a JVM without an agent, next to an attach budget of 10 s (`attach/mod.rs:43`) and a readiness wait of up to 3 s (`backend.rs:78`). *Inferred:* 0.1 to 0.5 s.
- **[The query has no deadline]** → As today. Standard input is closed (`Command::output`), the answer is small, and no cause of a hang is known (Open Questions).
- **[A broken package stays cached for the process]** → Reinstalling while a runtime runs takes effect in the next process, as for every lookup today. The warning's remedy says to reinstall, and the user runs again.
- **[Per-runtime warnings repeat per suite]** → One warning per suite that meets a JVM without an agent, each true for its suite. `robotcode results log --level WARN --execution-messages` lists them together.
- **[The attempt record is keyed by pid and never pruned]** → Existing behavior: a JVM under a recycled pid inherits spent attempts, and now the refusal flag too (Open Questions).
- **[Healthy lanes must stay free of warnings]** → The Windows lane installs the package (`just install-provider-java`), its versions match, and the fixture runs on Java 8, where nothing refuses. The JAB suites switch the agent off (`tests/acceptance/swing/resources/testapp.resource:32`), so nothing is resolved there. Task 9.2 checks the lane's warnings.
- **[A shared file]** → `xpath-document-order` edits `agent/backend.rs` in other functions. Whichever lands second rebases.

## Migration Plan

- **Behavioral:**
  - new warnings, listed in the proposal;
  - test runs outside an activated environment now find an installed package and attach the agent;
  - a JAR that cannot be resolved no longer spends a JVM's attempts.
- **Internal API**, of workspace crates only:
  - `JarUnavailable` gains `cause`;
  - `set_host_interpreter` replaces the unused `set_in_process_resolver`;
  - `installed_package` and `query_interpreter` return an outcome instead of an `Option`.
- **Native rebuild:** yes. `just build-native` for real use, `just build-native-mock` for the pytest.
- **The Java parts:** the agent JAR does not change, and `packages/provider-java` changes only in its docs, so their one version needs nothing new. The Windows lane still needs `just install-provider-java`.
- **Platforms:** the behavior changes on Windows. On Linux and macOS only the hand-over runs.
- **Sequence:**
  1. the tests;
  2. `java-agent`;
  3. `provider-java`;
  4. the extension, then the rebuild;
  5. the docs;
  6. verification on Linux, then on Windows, and the delivery check.
- **Rollback:** revert per commit. The reports (`provider-java`) match on the cause that the discovery commit introduces, so reverting the discovery commit means reverting the reports first. Reverting only the reports keeps the new discovery and brings back the silent debug records.

## Open Questions

- **Should a refusal end the attempts for its JVM?** Today a JVM gets up to three attaches per runtime. On Windows each is an `OpenProcess` plus `CreateRemoteThread`, and a JEP 451 refusal cannot change while the JVM runs. The spec here holds either way.
- **Should the interpreter query get a deadline**, and which? No hang is known today.
- **The standalone binaries** in a conda environment or a system-wide installation still find no environment to ask. Should their inference learn conda's `conda-meta` marker and the root `python.exe` on Windows, in a change of its own?
- **Should the attempt record be keyed by process identity** — `platynui-process`'s `ProcessIdentity`, a pid plus its start time — so that a JVM under a recycled pid starts fresh?
