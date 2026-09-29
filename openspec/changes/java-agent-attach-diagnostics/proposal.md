# Proposal

## Why

Automatic attachment is how the in-JVM agent reaches a Java application, and installing `platynui-provider-java` is the only consent it asks for. Today almost every way this can go wrong switches the agent off in silence. On Windows the user then sees the weaker Access Bridge tree; once `java-provider-linux` lands, a Linux user sees no tree at all. At the default level, nothing in the log says why.

This is follow-up A4 of the logging triage (`openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`, entries at `:522`, `:527`, `:599` and `:711`, and the dropped entries at `:566` and `:790`). Every finding below was checked again at HEAD on 2026-09-29:

- **A configuration or installation mistake is a debug line.** Every error of `resolve_agent_jar` is recorded at debug as "no agent JAR available; automatic attachment is off for this session" (`crates/provider-java/src/agent/backend.rs:283-289`). The errors were written to name their remedy — a configured `providers.java.agent.jar` or `PLATYNUI_JAVA_AGENT_JAR` that is not a file (`crates/java-agent/src/discovery.rs:97-101`, `:135-143`), an installed package of another version (`:106-115`) — but nobody sees them. The wording is wrong as well, because the resolution runs again on every pass. And since the attempt counter of a JVM is charged before the JAR is resolved (`backend.rs:272-277`), a bad JAR uses up each JVM's three attempts.
- **A JVM that refuses the agent is a debug line.** The attach transport keeps `AgentError::AgentRefused` apart from a failed attach precisely because its remedy lies inside the target (`crates/java-agent/src/error.rs:47-53`, produced at `attach/mod.rs:163` and `:177-181`). The provider then records it at debug like every other attach failure (`backend.rs:291-295`).
- **A broken installation is reported as a missing one.** The discovery query swallows every exception of an entry point (`discovery.rs:173-186`). `platynui-provider-java` raises `FileNotFoundError` on purpose when its JAR is missing from the installation (`packages/provider-java/src/platynui_provider_java/__init__.py:25-39`), and the user is told to install `robotframework-platynui[java]` — a package that is installed.
- **Robot Framework runs outside an activated virtual environment never find the package.** The discovery design relies on an in-process resolver for test runs (`discovery.rs:15-18`, `openspec/specs/java-agent/spec.md:69`, `packages/provider-java/README.md:29-31`). That resolver was never wired: `set_in_process_resolver` (`discovery.rs:79-81`) has had no caller since it was introduced in `09d97a63`, and `packages/native` never calls into discovery. A test run therefore infers its environment the way a standalone binary does (`discovery.rs:251-260`): from a `pyvenv.cfg` in the directory above the running executable, or from `VIRTUAL_ENV`. A conda environment and a system-wide installation have neither. There, automatic attachment is silently off, and the only diagnostic recommends installing the package they have. The lanes work only because `uv run` sets `VIRTUAL_ENV`.
- **A zero or negative agent call timeout falls back to the default in silence** (`backend.rs:127-131`), and a unit test pins that silence (`backend.rs:602-607`).

`java-provider-linux` needs this first. On Linux no Access Bridge serves a JVM that the agent cannot reach, and that change's spec requires that switched-off attachment "says so rather than implying a fallback exists".

## What Changes

- **The Python extension names its interpreter, and discovery asks it.** When `platynui_native` is imported, it records `sys.executable` for the agent's discovery. That is plain data: no package metadata is read, nothing Java-related happens. The first time a JAR is needed, discovery asks that interpreter in a child process, exactly as the standalone binaries already ask theirs. No Python code runs inside a runtime call. A test run in a conda environment, a system-wide installation or a virtual environment that was never activated therefore finds the package it has. The unused in-process resolver hook is removed.
- **Discovery tells its outcomes apart.** The query reports an entry point that raised, with the exception's type and text. The resolution distinguishes five outcomes, and only the first names installing the package as the remedy:
  - nothing installed;
  - a configured path that is not a file;
  - an installed package of another version;
  - an installed package that is broken;
  - an environment interpreter that could not be run or answered something unreadable. This is the dropped review entry at `discovery.rs:231`; the premise of its drop — that the in-process resolver answers first in Robot Framework — was false.

  `AgentError::JarUnavailable` gains a cause that says which.
- **Mistakes warn once; the absent package stays quiet.** When automatic attachment cannot get a JAR because of a mistake, the Java provider warns once per runtime for each distinct mistake, naming the cause and its remedy. It records repeats at debug, and records at debug when the JAR resolves again. An absent package is the consented default and is recorded once per runtime at debug. A JAR that cannot be resolved no longer costs a JVM its attach attempts.
- **A refusing JVM is named once, with its remedy.** A JVM that refuses the agent — dynamic agent loading disallowed (JEP 451), or the agent library declined — is warned about once per JVM and runtime. The warning names `-XX:+EnableDynamicAgentLoading` (which `JAVA_TOOL_OPTIONS` can carry), `-javaagent` at launch, and `providers.java.agent.auto_attach = false`. A failed attach, a process that is not a JVM and a process that is gone stay at debug. A JVM that does not answer is `jab-discovery-containment`'s report.
- **The wording is platform-neutral.** No record says or implies that another backend serves the application, because on Linux none does. The consequence is stated as "not served through the agent".
- **A zero or negative `providers.java.agent.call_timeout_ms` warns**, naming the key and the value; the default applies as before. The Access Bridge's own `call_timeout_ms` is not part of this change.

Behavior changes for the release notes (PlatynUI is 0.x, so none of this is framed as breaking):

- Robot Framework runs outside an activated virtual environment — a conda environment, a system-wide installation, a virtual environment's interpreter started directly — now find an installed `platynui-provider-java`. Their Java applications are therefore served through the agent, as documented, instead of only through the Access Bridge. The agent's tree differs from the bridge's (a table has rows, for example). `providers.java.agent.auto_attach = false`, or uninstalling the extra, keeps the previous behavior.
- New warnings:
  - a configured JAR path that is not a file;
  - a broken or mismatched installation of `platynui-provider-java`;
  - an environment interpreter that cannot be asked;
  - a JVM that refuses the agent;
  - an agent call timeout of zero or less.

Out of scope, each in its own change: the entries about the Access Bridge DLL discovery, the JAB timeouts (including the JAB half of the `call_timeout_ms` entry), the `SwingElement` defects, and the agent's notification queue (`crates/provider-java/src/agent/session.rs:144`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `java-agent`: *Delivery as an opt-in package* — discovery asks the interpreter the runtime runs in instead of resolving in-process, and every cause other than an absent package names its own remedy, never the install.
- `java-provider`: ADDED *Automatic attachment that cannot happen says why* — the reporting levels, once per runtime for each mistake and for each refusing JVM, the platform-neutral consequence, and attempts that an unresolvable JAR does not use up.
- `diagnostic-logging`: *Configuration mistakes are reported* — an agent call timeout of zero or less is warned about.

## Impact

- **Rust, `crates/java-agent`** (portable; `just test-crate platynui-java-agent` runs on every OS):
  - `src/discovery.rs`: the interpreter hand-over replaces `set_in_process_resolver`; the query reports entry-point failures; the lookup and the resolution get distinct outcomes; the module docs.
  - `src/error.rs`: `JarUnavailable` gains its cause. `src/attach/mod.rs`: its own JAR checks set it.
  - `tests/delivery.rs`: follows the richer outcome and gains the broken-installation case. `tests/live_fixture.rs`: a live refusal check on a JDK 21 fixture.
- **Rust, `crates/provider-java`** (Windows-gated today), `src/agent/backend.rs`:
  - the attach pass: resolution before attempts are charged, the reports, and a test seam;
  - the timeout warning;
  - their unit tests with log capture, replacing the test that pins the silent default.
- **Rust, `packages/native`:** depends on `platynui-java-agent` on every platform; names its interpreter at import, in a small module next to `log_bridge.rs`; gains two test hooks in the mock build.
- **Python:** no library change. A new pytest, `packages/native/tests/test_java_agent_discovery.py`. `packages/provider-java` changes only in its docstrings, its README and a `pyproject.toml` comment.
- **Robot Framework:** no keyword change and no new suite. The behavior lives in the provider, and the Windows lane's agent suites and its warning check cover it.
- **Docs:**
  - `dev-docs/java-toolkits.md` (*Getting the agent*);
  - `dev-docs/platform-windows.md` (automatic attachment);
  - `dev-docs/python-bindings.md` (what the extension does at import);
  - `dev-docs/logging.md` (§2 and §9);
  - the status lines of the A4 entries in the logging review.
- **Native rebuild:** yes. The agent JAR does not change, and `packages/provider-java` changes only in its docs, so the one version of the three Java parts needs nothing new. `just install-provider-java` is needed on the Windows lane, as before.
- **Platforms:** Windows today, where the Java provider is built. On Linux and macOS the extension records its interpreter, but nothing reads it until `java-provider-linux`.
- **Coordination:**
  - `java-provider-linux` lands after this change. Its switched-off-attachment diagnostic builds on the platform-neutral wording here, and the Linux dependency sections it adds to `packages/native` find `platynui-java-agent` already present.
  - `jab-discovery-containment` is independent. It changes the router (`crates/provider-java/src/backend.rs:51-77` and `provider.rs`), not `agent/backend.rs`. Its held JVMs join the attach candidates, and a failed attach to one of them stays at debug here, so its "does not answer" warning stays the only report.
  - `xpath-document-order` also edits `agent/backend.rs`, in the enumeration (`:384-411`) and the hit-test chain (`:498-514`). Whichever lands second rebases.
