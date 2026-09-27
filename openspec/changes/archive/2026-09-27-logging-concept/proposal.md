# Proposal

## Why

Since `native-log-bridge`, every native warning becomes a Robot Framework warning on the console and under *Test Execution Errors*, and native debug output can be switched on for a run. That turns the project's logging into user interface, and a project-wide review (seven area surveys, each verified against the code; 242 proposals, 201 confirmed, 25 added by the verifiers) found that it is not ready for that:

- **Repeated warnings flood the console.**
  - A provider that fails to list its top-level elements logs `error!` on every desktop enumeration (`crates/runtime/src/runtime/desktop.rs:134`). A `Wait Until …` poll enumerates up to ten times a second, and the message does not name the provider.
  - A D-Bus timeout warns on every affected property read and names neither the call nor the application (`crates/provider-atspi/src/timeout.rs:67`).
  - The Java agent's version-mismatch warning repeats on every enumeration (`crates/provider-java/src/agent/backend.rs:177`).
- **Failures are reported twice.** `error!` is emitted for failures that are also returned as `Err`, so a Robot Framework user sees an ERROR plus the keyword's exception. The rule document's own examples are such cases (`.github/instructions/tracing.instructions.md:37`).
- **Some failures stay silent:**
  - a failed auto-activation before an action ends in `pass` (`src/PlatynUI/BareMetal/__init__.py:1649`);
  - wrongly typed values in pointer and keyboard profiles are dropped (`packages/native/src/runtime.rs:2819` and four siblings);
  - a wrongly typed provider setting falls back to its default (`crates/core/src/config.rs:113`);
  - a keyboard error can leave keys held down without a trace (`crates/runtime/src/keyboard.rs:44`);
  - `PLATYNUI_LOG_LEVEL=WARNING`, the natural Python spelling, is read as a target filter and switches every native warning off (`packages/native/src/log_bridge.rs:206`; `tracing-core` accepts no `warning`).
- **Decisions do not name what was decided.** The runtime reports a provider count and `platform = true`, never which backend or providers it chose (`crates/runtime/src/runtime/mod.rs:159,238`).
- **Keywords leave no trace of what they did.** No action keyword says which element it hit or where a click landed, and error messages call elements `UiNodeDescriptor`.
- **Typing errors repeat the input and do not say where.** The sequence parse error prints the whole input line (`crates/runtime/src/keyboard_sequence.rs:16-17`). A character or key the backend cannot convert is named without its position, and `pa<ss>wd` fails with `unsupported key: ss` and no hint that `<` needs escaping (`:96-110`). The keyboard keywords do not accept Robot Framework's `Secret`.
- **The levels do not tell the reader what to do.** The only level table is for Rust (`.github/instructions/tracing.instructions.md:33-41`). Its `warn` also covers fallbacks and slow calls that are normal in some sessions, so a healthy run can print warnings. Its `info` allows one record per program run, which does not fit resources that come and go. `error` is used for failures that are also returned (above).
- **The rules cover only Rust.** Python has none, the command-line tool, the Inspector and the Python extension each build their log filter from their own copy, and several documents contradict the code.

A unified concept, the shared building blocks it needs, and the 26 high-priority findings of the library are the first step. The review's medium- and low-priority findings follow in separate changes, one per area.

## Scope

The concept covers the PlatynUI library, and the tools shipped with it:

- the native core, the platforms and the providers;
- the Python bindings and the Robot Framework library;
- the command-line tool and the Inspector.

The Wayland compositor, the EIS test client and the egui test app are development and test tools. The library never starts them; the compositor exists because no available compositor offers the APIs test automation needs. They are outside this change. Their findings, four of them of high priority, are kept in `review-findings.md` for a separate change. The acceptance lane that runs inside the compositor stays, as the place where the library is tested on Wayland.

## What Changes

### The levels

Every level gets one meaning, the same in Rust and in Python. Every record this change adds or touches is placed by it; the follow-up changes in `review-findings.md` place the rest. Who sees a record follows from its level: native records reach Python `logging` and the Robot Framework log through the bridge of `native-log-bridge`, at the corresponding RF level.

| Level | Meaning | Who sees it |
|---|---|---|
| error | Something PlatynUI had to do failed unexpectedly and the failure was swallowed, so its results or the system's state are wrong, and nothing else reports it. States the consequence. Never for a failure that is returned or raised. A condition that recurs on every call is reported once per episode. | By default: the RF console and *Test Execution Errors*; stderr of the command-line tools |
| warn | PlatynUI knowingly works with less than it was asked for, because of the environment, the configuration or the target application, and says what the user loses. Never in a healthy session, never for a failure that is returned or raised. A condition that recurs on every call, a failure or a subject that stays slow, is warned once per episode. | By default, like error. A level of `error` (native or RF `--loglevel`) hides them. |
| info | A lifecycle transition of a long-lived native resource, once per transition: a runtime is created or shut down, the Java agent is injected into a JVM. Never per operation. PlatynUI's Python code logs no diagnostic at info, because Robot Framework shows Python INFO records by default. | Native records only when switched on (`native_log_level=info`, `--log-level info`, `PLATYNUI_LOG_LEVEL=info` or `RUST_LOG`); RF shows them at its default `--loglevel INFO`. |
| debug | What a single operation did or decided, fallbacks normal in some sessions, a call slower than its call class's threshold, returned failures with their context, keyword action lines. | Native records only when switched on to debug, and in RF also at `--loglevel DEBUG`; keyword action lines at RF `--loglevel DEBUG` alone. |
| trace | One record per element, tick, key or character, or message. A record that names a key, character, key code or keysym, or carries a keyboard device's error text, is trace-level however often it occurs. | Only when switched on to trace, and in RF at `--loglevel TRACE`. |

A keyword's own output, such as an embedded screenshot, stays at RF's INFO. It is the keyword's result, not a diagnostic.

What this change does at each level:

- **error**
  - Once per episode and by name, instead of on every enumeration: a provider that fails to list its elements.
  - New: keys that may stay held because releasing them failed.
  - Moved to debug, because the failure is returned: the AT-SPI connection errors, Windows pointer and screenshot, X11 screenshot.
- **warn**
  - Once per episode instead of per call: AT-SPI timeouts, per application; the Java agent's version mismatch, per process.
  - Reworded to name the cause and the consequence: no platform backend (today a warning about a fallback desktop that names no backend; now naming the backends tried and what stops working) and no Wayland input backend (now with each backend's reason).
  - New where there was silence: no active provider, saying that queries find only the desktop node; a pointer target clamped to the desktop; configuration mistakes; `PLATYNUI_LOG_LEVEL` set to a word that is no level, such as `verbose`; the test build used for real work; the macOS stub; `platynui-cli watch` without an event source.
  - Moved to when it matters: the missing JAB DLL, which today warns once in every Windows run without the bridge, is debug at startup, and warned once per process by the Java provider's router when a Swing or AWT window is found that no Java backend serves.
  - Moved to debug, because the failure is returned: the warning before releasing keys after a keyboard error; the D-Bus timeout warning for calls whose timeout the AT-SPI provider returns (connecting, reading the registry), which the enumeration's error reports instead.
- **info**
  - Extended: the runtime's existing creation record, which today gives only a provider count, names the chosen backend, whether it was forced, the provider ids, the desktop bounds and the monitor count.
  - Not used for keyword narration (maintainer decision: DEBUG).
- **debug**
  - New: one line per action keyword (below); a failed automatic activation and a UIA activation that did not reach the foreground (maintainer decision); the alternatives a decision rejected; returned failures with their context; the quiet counterpart of a new warning: `platynui-cli watch` when a provider does emit events.
- **trace**
  - Moved here from debug: records that name individual keys (X11, mock) and mock pointer moves.

### Everything else

- **One logging concept for Rust and Python** (`dev-docs/logging.md`). It sets out:
  - the levels and who sees them;
  - where a failure is logged: once, by the layer that swallows it or decides its consequence;
  - once per episode for warnings and errors that can recur on every call;
  - the message style and the field names;
  - one node description;
  - the Python mechanisms: `logging` under `platynui.<module path>`, keyword modules included; `robot.api.logger` only for keyword output that needs Robot Framework; `warnings.warn` only for the Python API;
  - typed text and `Secret`;
  - how to look at a run's warnings with the existing tools.

  `.github/instructions/tracing.instructions.md` becomes `.github/instructions/logging.instructions.md`, a short checklist for `**/*.rs` and `**/*.py` that links the concept. The contradicting documents are corrected, and the user-facing documentation of `PlatynUI.BareMetal` gains a short "Reporting a problem" paragraph.
- **Shared building blocks in `platynui-core`, without a `tracing` dependency:**
  - a keyed transition latch ("first failure", "still failing", "recovered") that call sites use to warn once per episode;
  - a node description (`Role "Name" #Id`, one line, at most 60 characters of name) used in new log fields, keyword log lines and error messages, and available from Python as `UiNode.describe()`. The element's `repr` stays free of provider calls.
- **One level knob for every entry point.**
  - A new small crate builds the filter for the CLI, the Inspector and the Python extension.
  - A single level from `--log-level`, `native_log_level` or `PLATYNUI_LOG_LEVEL` means `warn,platynui=<level>` for `warn`, `info`, `debug` and `trace`, and applies to every module for `error` and `off` (maintainer decision). **Behavior change:** `--log-level debug` no longer opens third-party crates such as zbus and wgpu; `RUST_LOG` does.
  - `RUST_LOG` is the only source of filter directives. **Behavior change:** `PLATYNUI_LOG_LEVEL` takes a single level only.
  - Python-style level names (`WARNING`, `CRITICAL`) are understood. A value that is no level is rejected and reported once, and the next source applies, instead of silently filtering everything.
  - The Inspector forwards the `log` records of its GUI stack into the same filter.
- **Keyword action lines at DEBUG.** Every action keyword of `PlatynUI.BareMetal` logs one DEBUG line after success, naming the element, the resolved point or strategy and the parameters, never the text it types. It describes the element before it acts, and only when DEBUG is on, so the default log costs no provider call. Maintainer decision: DEBUG, so the default RF log stays as it is.
- **Keywords do not repeat the text they type** (maintainer decision: PlatynUI is a test system, so read values and key conversions may be reported, but Robot Framework already records the arguments).
  - A typing error says why: the position in the text the keyword received, the character or key that could not be converted, and the backend's reason. A parse error gives the position and what is wrong there in the user's terms, such as a `<` that is never closed, instead of the whole input. For a plain string it adds the Robot Framework form of an escape (`\\<`).
  - The keyboard keywords accept Robot Framework's `Secret` when RF ≥ 7.4 is installed. It is imported conditionally; the minimum stays RF 7.0. For a `Secret`, an error gives the position and the kind of failure, and no part of the value. A `Secret` uses the same sequence syntax as a string; the lenient backslash rule stays (maintainer decision).
  - Keyboard records that name keys move from debug to trace, because a record that names a key is trace-level however often it occurs, so nothing up to debug that PlatynUI derives from a `Secret` shows one of its keys. Values read back from the application are not affected.
- **Configuration that cannot be used is reported.** Each component checks its own settings where it reads them, before it builds anything (maintainer decision).
  - An unknown setting key under a component that is actually running warns. This is a change to `runtime-session-config`: unclaimed component ids of other platforms stay debug.
  - A misspelled top-level key such as `platfrom`, a bucket that is not a dict, and a value the Python binding cannot pass on warn.
  - A provider or platform setting of the wrong type warns and uses the default.
  - A pointer or keyboard profile value of the wrong type raises `TypeError`, like its sibling fields already do. **Behavior change:** such a value was silently ignored before.
  - An unknown profile key warns.
- **The 26 high-priority findings of the library.** The review rated 30 findings high; the four about the Wayland compositor are outside the scope. Most of the 26 are the level changes above; the level names, the Inspector's `log` forwarding and the profile `TypeError` are in the bullets above. The others:
  - the Java provider treats an agent answer that carries an error as proof the agent is alive, so only timeouts and transport, protocol and no-agent errors mark the JVM degraded;
  - the Windows attach stub is leaked instead of freed while its remote thread may still run (a crash fix);
  - not-found errors say whether the element or the root set by `Set Root` was missing.
- **Records arrive at once.** `Runtime(...)`, `Runtime.new_with_mock()` and `Runtime.shutdown()` deliver queued native records before they return, so a warning such as the test-build warning reaches Python without waiting for the next call.
- **Assertion failures without a message no longer start with `None`** (`src/PlatynUI/_assertable.py:121`).
- **Python logger names follow the package path in lower case.** `platynui.devices` becomes `platynui.core.adapter_devices`, and BareMetal logs through `platynui.baremetal`. **Behavior change** for anyone who configured the old name.
- **No lane report.** Robot Framework and robotcode already show every warning of a run; the rule that a healthy run has no PlatynUI warning is checked with `robotcode results log --level WARN --execution-messages` (maintainer decision).

## Prerequisite

This change modifies the `native-logging` spec that `native-log-bridge` introduces. That change must be archived first, so that `native-logging` exists as a main spec.

## Capabilities

### New Capabilities

- `diagnostic-logging`: what PlatynUI's diagnostics promise to users of the library, the CLI and the Python bindings. This covers:
  - one meaning per level, and who sees each level;
  - warnings that are actionable and not repeated per operation;
  - no double reporting of returned failures;
  - decisions that name their outcome;
  - keyword action lines at DEBUG;
  - one node description;
  - fallbacks that change an action's effect;
  - keywords that do not repeat the text they type, and `Secret` support;
  - one meaning of the level knob across entry points;
  - reported configuration mistakes;
  - capabilities that are missing, said once when they matter.

### Modified Capabilities

- `native-logging`: *Only native warnings and errors are produced by default*. The requirement refers to diagnostic-logging's *The level setting means the same everywhere* for the sources, the level names and their meaning, and keeps what is specific to the bridge: `native_log_level` as the requested level, the import failure for an invalid value, and the process-wide most-verbose request. It keeps the six scenarios of the main spec, narrows the invalid-environment scenario to a value that is not a level with no requested level, and adds the Python spelling `WARNING`.
- `jab-provider`: *Provider registration and inert absence*. A missing Access Bridge DLL is recorded at debug and becomes a warning only when a Swing or AWT window is found that no Java backend serves.
- `java-app-classification`: *Cross-platform enablement diagnostic*. A missing DLL is reported once per process with the DLL remedy instead of the `jabswitch` hint; an agent-served window is not reported.
- `runtime-session-config`: *Unclaimed configuration keys are tolerated* changes. A setting key that a claimed, active component does not recognize is now recorded as a warning, and so are a top-level key other than `platform`/`providers` and a bucket that is not a dict. Unclaimed component ids and foreign-platform blocks stay tolerated at debug.

## Impact

- **Rust crates:**
  - `crates/core` gains the transition latch and the node description (no new dependency).
  - A new `crates/log-filter` holds the shared filter building; the extension, the CLI and the Inspector use it.
  - `crates/runtime`, `crates/platform-linux` (X11/Wayland factories), `crates/provider-atspi`, `crates/platform-linux-x11`, `crates/platform-linux-wayland`, `crates/provider-java`, `crates/provider-java-jab`, `crates/java-agent`, `crates/provider-windows-uia`, `crates/platform-windows`, `crates/provider-macos-ax`, `crates/platform-mock` and `crates/provider-mock` get the high-priority fixes. Every PlatynUI platform and provider checks the configuration keys it reads.
  - `crates/cli` and `apps/inspector` switch to the shared filter. The development tools keep their own setup (see Scope).
  - For typed text: `crates/runtime` adds positions to keyboard errors; the per-key records of `platform-linux-x11` and `platform-mock` move to trace.
- **Python:**
  - `src/PlatynUI/BareMetal` logs through `platynui.baremetal` and gains keyword DEBUG lines, `Secret` support, the DEBUG for failed auto-activation, clearer not-found errors and updated user documentation.
  - `src/PlatynUI/_assertable.py` no longer starts a failure without `assertion_message` with `None`.
  - `src/PlatynUI/core` gets the renamed loggers.
  - `packages/native` gets the profile extractors, the configuration warnings of `parse_runtime_config`, `UiNode.describe()`, log delivery at construction and shutdown, the shared filter and a `sensitive` flag on the keyboard calls.
- **Docs and rules:**
  - new `dev-docs/logging.md`;
  - `.github/instructions/tracing.instructions.md` becomes the checklist `.github/instructions/logging.instructions.md`;
  - the user-facing documentation in `PlatynUI.BareMetal` and the native docstrings;
  - `.github/copilot-instructions.md`, `dev-docs/error-handling.md`, `dev-docs/keyboard-input.md` §6, `dev-docs/python-library-design.md` §A.9.7, `dev-docs/python-bindings.md` and `dev-docs/plan-waylandCompositor.md` are corrected; AGENTS.md lists `dev-docs/logging.md` among its design docs;
  - CLI help texts are fixed.
- **Specs:** new `diagnostic-logging`; modified `native-logging`, `runtime-session-config`, `jab-provider` and `java-app-classification`.
- **Deferred findings:** the review's 196 other findings, and the four compositor findings of high priority, are kept in `review-findings.md` next to the design, per area, for the follow-up changes.
- **Tests:**
  - core unit tests for the latch and the node description;
  - unit tests for the shared filter;
  - mock RF suites and pytest for the keyword DEBUG lines, `Secret`, failed activation and configuration reports;
  - Rust tests for each fix where the path is reachable without a display, including a stub keyboard device for the position errors.
  - Real-provider behavior is checked in the X11 and compositor lanes. Windows and macOS changes are verified on those machines before archiving, or moved to a follow-up change.
- **Native rebuild needed.** The Python-visible parts (`UiNode.describe()`, profile extraction, the filter, constructor delivery and the `sensitive` keyboard argument) live in the extension.
- **Platforms:** all. Windows-only fixes (UIA activation, JAB, attach stub) and the macOS stub warning need their platforms for verification.
