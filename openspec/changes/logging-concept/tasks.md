# Tasks

## 0. Prerequisite

- [x] 0.1 Archive `native-log-bridge` first (`/opsx:archive native-log-bridge`). This change modifies its `native-logging` spec, which becomes a main spec only on archive. Verify `openspec/specs/native-logging/spec.md` exists and `openspec validate logging-concept --strict` reports no archive refusal for `native-logging`.

## 1. The concept and the rules

- [ ] 1.1 Write `dev-docs/logging.md` (design decisions 1–11) as normative, explanatory prose with examples. Cover:
  - the channels: native diagnostics, and the Python library's own records including the keyword lines; plus a short paragraph on diagnostics inside a target JVM that links `dev-docs/java-toolkits.md` and `AgentLog` instead of restating them;
  - the level table and who sees each level (proposal, *The levels*), including that PlatynUI's Python code logs no diagnostic at info;
  - slow calls: debug per call against one threshold constant per call class, a warning once per episode for a subject that stays slow;
  - log-or-return, including where an enumeration failure is reported;
  - once per episode, with the latch, and that the end of an episode is recorded once at debug;
  - decisions name their outcome;
  - external boundaries (`call`, target, `elapsed_ms`, `timeout_ms`);
  - the message style and the field names of design decision 1, with its worked example;
  - producer rules: a native component logs under a target that starts with `platynui`; diagnostics are events and carry their context in fields, because spans are not carried to Python or Robot Framework; expensive message parts are built only when their level is enabled;
  - the element description and why `repr` stays cheap;
  - the Python mechanisms and logger names (design decision 9);
  - typed text: keywords do not repeat it, errors say why with a position, `Secret` support and its sequence syntax, keyboard records that name keys only at trace (which shows the keys of a `Secret` too), that read values and conversions may be reported, and that Robot Framework records arguments as written and their values at TRACE, where only a `Secret` is hidden;
  - the level knob and its behavior changes;
  - how to look at a run's warnings (`robotcode results log --level WARN --execution-messages`), and that a healthy run has none from PlatynUI.

  It states rules, not the review's counts. Verify it names every requirement of `diagnostic-logging`.
- [ ] 1.2 Replace `.github/instructions/tracing.instructions.md` with `.github/instructions/logging.instructions.md` (`applyTo: '**/*.rs,**/*.py'`), a checklist that links `dev-docs/logging.md`. It contains:
  - §1 (dependencies), with the feature sets of `platynui-log-filter` (`fmt` for the CLI and the Inspector, `log` for the Inspector only, with the reason);
  - §4 (import style, structured fields, field formatting, the naming pitfall), with examples that follow the message style and log-or-return: `error = %err` instead of `%err` (`:143`), and no warning for a returned failure (`:127-128`);
  - one line per level;
  - one line each for log-or-return, once per episode through `platynui_core::diagnostics::Transitions`, records that name a key, character, key code or keysym at trace only, and the element description through `describe`;
  - one line each for the producer rules: a native component logs under a target that starts with `platynui` (or the level knob does not reach it); diagnostics are events with fields, not spans; expensive message parts are built only when their level is enabled;
  - the entry-point rule: `platynui_log_filter::init_stderr`, and `value_parser = platynui_log_filter::parse_level` for `--log-level`;
  - the Python lines: loggers `platynui.<module path>` through `logging`, keyword modules included; `robot.api.logger` only for keyword output that needs Robot Framework; no diagnostic at INFO; `warnings.warn` only for the Python API.

  The per-layer guide (§5) is dropped; its guidance becomes examples in the concept. Update the references:
  - `dev-docs/architecture.md:768`;
  - `dev-docs/python-bindings.md:71`;
  - `.github/copilot-instructions.md:117-118`: replace the subscriber sentence and the level summary with one line that points to `.github/instructions/logging.instructions.md` and `dev-docs/logging.md` (the Python extension installs a queueing subscriber too, the binaries do so through `platynui-log-filter`, and fallbacks that are normal in some sessions are debug, not warn);
  - AGENTS.md's Design Docs list gains `dev-docs/logging.md` (levels, log-or-return, once per episode, Python loggers, the level knob).

  Verify with `grep -rn "tracing.instructions" --exclude-dir=openspec --exclude-dir=target --exclude-dir=.venv --exclude-dir=.git .` that nothing points to the old name.
- [ ] 1.3 Correct the contradicting developer documents:
  - `dev-docs/python-library-design.md` §A.9.7 (`:3525-3532`) becomes a pointer to the concept, with an English summary line; `:3452` names `platynui.core.adapter_devices`;
  - `dev-docs/python-bindings.md`: the Logging section keeps the bridge mechanics and links the concept; `:40` says that an unknown top-level bucket, a bucket that is not a dict and an unknown setting key of an active component are reported as warnings, that a non-string key and a value of an unsupported type are warned about with their dotted path and Python type, that a wrongly typed setting is warned about by its component, which applies its default, and that unclaimed component ids stay debug; `:52` names `platynui.core.adapter_devices`; `:64-67` say that `PLATYNUI_LOG_LEVEL` takes a single level with the same meaning as the requested level, that `RUST_LOG` is the only source of directives, and that `error` and `off` apply to every module while `warn` to `trace` lower only PlatynUI's own crates; `:69` says that a rejected value is reported once per process, naming the variable and the value;
  - `dev-docs/error-handling.md:176`: "If both are useful, record the context at debug and return the typed error; only the layer that swallows a failure or decides its consequence logs it above debug (see `dev-docs/logging.md`).";
  - `dev-docs/keyboard-input.md` §6: `\x` and `\u` need exactly 2 or 4 hex digits, otherwise the sequence fails with the escape and its position (`C:\users` fails; `C:\\users` types `C:\users`); a backslash at the end of the text or before a line break fails; a backslash before any other character is dropped. Remove "a stray backslash is never an error".
  - `dev-docs/plan-waylandCompositor.md:1193`: a missing highlight backend fails with `CapabilityUnavailable`.

  Verify each statement against the code it describes.

## 2. Failing tests first — building blocks

- [ ] 2.1 Add unit tests in `crates/core` for the not-yet-existing `diagnostics::Transitions<K>` (design decision 3):
  - `Started` then `Continuing`;
  - `recovered` re-arms, and is false for a key that never failed;
  - independent keys;
  - `retain` forgets keys;
  - concurrent `failed` calls start exactly one episode;
  - `failed` and `recovered` accept a borrowed key, such as `&str` for `String` keys.

  Verify `just test-crate platynui-core` fails to compile.
- [ ] 2.2 Add unit tests in `crates/core` for `ui::describe` with stub nodes (design decision 4):
  - `Button "OK"` without id;
  - `Button "OK" #ok` with id;
  - an id that is empty after trimming is treated as absent;
  - quotes, backslashes and a line break are escaped, so the description is one line;
  - a name of 61 characters gives its first 59 characters and `…`, also for non-ASCII characters, without a panic;
  - an empty name as `""`;
  - a name of 61 characters with a line break at position 59 ends in `\n…`, because the name is cut before it is escaped.

  Verify they fail to compile.
- [ ] 2.3 Create `crates/log-filter` (package `platynui-log-filter`) with tests only (design decision 5).

  `parse_level`:
  - is case-insensitive;
  - accepts `off`;
  - maps `warning` to WARN and `critical`/`fatal` to ERROR;
  - returns an error for anything else, whose message reads "unknown log level 'verbose'; expected one of off, error, warn (warning), info, debug, trace, critical, fatal".

  `filter_spec`:
  - `RUST_LOG=zbus` and `RUST_LOG=platynui_runtime=trace,warn` are used verbatim;
  - `RUST_LOG=zbus=loud` is rejected, named and counts as absent: with a requested debug level the result is `warn,platynui=debug`;
  - an empty or blank `RUST_LOG` or `PLATYNUI_LOG_LEVEL` counts as absent and is not reported;
  - a requested debug level gives `warn,platynui=debug`; a requested `error` gives `error`, and `off` gives `off`;
  - `PLATYNUI_LOG_LEVEL=debug` gives `warn,platynui=debug`, `WARNING` gives `warn,platynui=warn`, and `off` gives `off`;
  - `PLATYNUI_LOG_LEVEL=platynui_runtime=trace` and `PLATYNUI_LOG_LEVEL=verbose` are rejected, named with the hint to use `RUST_LOG` for directives, and count as absent;
  - the precedence holds;
  - with nothing set, the result is `warn`;
  - a rejected (variable, value) pair is reported once per process.

  Add filter tests proving that `warn,platynui=debug` lets `platynui_provider_atspi` debug through and not `zbus` debug, and that `error` hides a `zbus` warning. Verify `just test-crate platynui-log-filter` fails to compile.

## 3. Building blocks

- [ ] 3.1 Implement `platynui_core::diagnostics::Transitions<K>` and `platynui_core::ui::describe` without a `tracing` dependency. Verify:
  - 2.1 and 2.2 pass;
  - `cargo tree -p platynui-core -e normal --prefix none | grep -c '^tracing '` prints `0`.
- [ ] 3.2 Implement `crates/log-filter` with:
  - `parse_level(&str) -> Result<LevelFilter, UnknownLevel>`, where `UnknownLevel` implements `Error` and `Display`, so that it serves directly as a clap `value_parser`;
  - `filter_spec`, reporting each rejected (variable, value) pair once per process;
  - the `fmt` feature: `init_stderr(requested)`, using `try_init` and reporting rejected values and an init failure without panicking;
  - the `log` feature, which enables `tracing-subscriber/tracing-log` and lets `try_init` install `LogTracer`, with no manual `LogTracer::init`.

  Declare dependencies per crate. Add the crate to AGENTS.md's crate list and `platynui-log-filter` to `windows_rust_packages` and `macos_rust_packages` in the justfile. Verify 2.3 passes and `just cross-target-checks` builds it.
- [ ] 3.3 Switch the extension to `platynui-log-filter`:
  - move `filter_spec` and its tests out of `packages/native/src/log_bridge.rs`, and use `parse_level` in `set_log_level`;
  - make `Runtime(...)`, `Runtime.new_with_mock()` and `Runtime.shutdown()` deliver queued records before they return or raise (`shutdown` after releasing the lock) (design decision 12);
  - in `src/PlatynUI/core/native_logging.py`, normalize `native_log_level` case-insensitively (`warning`→`warn`, `critical`/`fatal`→`error`), accept `off` and rank it below `error`, before ranking;
  - update the `set_log_level` docstrings (`packages/native/python/platynui_native/_native.pyi:674-680`, `log_bridge.rs:411-417`);
  - update `tests/PlatynUI/test_native_log_levels.py:64` and `tests/PlatynUI/test_native_logging_rf.py:109`.

  Add pytest cases:
  - `PLATYNUI_LOG_LEVEL=WARNING` acts like `warn`;
  - `PLATYNUI_LOG_LEVEL=debug` produces a native debug record through the emitter;
  - `verbose` is rejected by name, and two request/withdraw cycles with `PLATYNUI_LOG_LEVEL=verbose` deliver exactly one warning;
  - `native_log_level=WARNING` imports and produces only warnings and errors;
  - `native_log_level=off` imports and produces no PlatynUI record;
  - at `native_log_level=info`, `rt.shutdown()` delivers its record before it returns.

  Verify with `just test-crate platynui_native` and `just test-python`.
- [ ] 3.4 Switch the CLI and the Inspector to `init_stderr`:
  - `crates/cli/src/lib.rs:44-74`;
  - `apps/inspector/src/lib.rs:559-579` (`log`).

  The development tools keep their own setup. Give both `--log-level` arguments an optional `value_parser = platynui_log_filter::parse_level`. Fix the CLI help (`crates/cli/src/lib.rs:79-83`) and the Inspector's help text to the new meaning. Verify with the CLI in the sidecar harness:
  - `--log-level debug info` and `PLATYNUI_LOG_LEVEL=debug` show `platynui_*` debug records and no `zbus` debug records;
  - `--log-level warning` and `PLATYNUI_LOG_LEVEL=WARNING` behave like `warn`;
  - `--log-level error` shows no `zbus` warning;
  - `--log-level verbose` exits with status 2 and the unknown-level message;
  - `PLATYNUI_LOG_LEVEL=verbose` prints the rejection warning;
  - `RUST_LOG=zbus` shows zbus records.

  Also verify the Inspector: a test in `crates/log-filter`, built with `--features fmt,log` in its own test binary, shows that after `init_stderr(Some(DEBUG))` a `log::debug!` record under a `platynui_*` target is emitted and one under another target is not; an argument test shows that the Inspector's `--log-level WARNING` parses and `--log-level verbose` is rejected; and in the X11 session, the Inspector started with `RUST_LOG=eframe=debug` prints eframe's "Using the … renderer" record on stderr, while with `--log-level debug` alone it prints `platynui_*` debug records and no `eframe` record.
- [ ] 3.5 Expose `UiNode.describe()` in `packages/native/src/runtime.rs` and `_native.pyi`, and give `__repr__`/`__str__` a form without provider calls that shows the runtime id (design decision 4). Add pytest cases against the mock:
  - `node.describe() == 'Button "OK"'` for the OK button;
  - `repr(node)` contains its runtime id.

  Verify with `just test-python`.

## 4. Failing tests first — configuration, keyboard and the Python library

- [ ] 4.1 Add Rust tests for configuration reporting (design decision 8). Add `tracing-subscriber` (`fmt`) as a dev-dependency of `platynui-runtime`, as `provider-atspi` has. Cover:
  - `ConfigMap::unknown_keys(known)` lists the keys not in `known` and always treats `enabled` as known;
  - `ConfigMap::try_bool/try_i64/try_str/try_map` return `Ok(None)`, `Ok(Some)` or `Err(ConfigTypeMismatch { key, expected, found })`;
  - a runtime with `config={'platform': {'backend': 'mock', 'mock': {'bogus': 1}}}` logs one warning naming `platform.mock` and `bogus`;
  - `platform.windows.bogus` on the mock backend logs debug only;
  - `platform.backend = 1` warns naming `platform` and `backend`;
  - `AtspiFactory::build` with `surface_popups='False'` warns naming `providers.atspi`, `surface_popups`, bool and str (`crates/provider-atspi/src/lib.rs:67-77`; no bus needed);
  - the X11 factory with `display = 1` warns naming `platform.x11` and `display`, although the build then fails for lack of an X server, because the check runs first.

  Verify they fail today.
- [ ] 4.2 Add pytest cases in `packages/native/tests`:
  - the profile extractors: `{'after_click_delay_ms': '100ms'}` raises `TypeError` naming the key and "number"; `{'speed_factr': 2}` produces one warning naming `speed_factr` through the log bridge, and the call proceeds;
  - the configuration binding, in the mock build with `{'platform': {'backend': 'mock'}}`: an added top-level key `platfrom` warns naming it and the accepted buckets `platform` and `providers`; `{'providers': 'atspi'}` warns naming `providers` and `str`; a non-string key such as `{'platform': {'backend': 'mock', 1: {}}}` warns naming the key and `int`; a leaf of an unsupported type such as a `pathlib.Path` warns naming its dotted path and the Python type.

  Verify they fail today.
- [ ] 4.3 Add runtime unit tests for typed text (design decision 7), with a stub keyboard device that rejects one character, rejects every multi-character name except `Ctrl`, and can fail sending. Use fragments that cannot occur in a message by chance, such as `Qz7`. Positions are 1-based characters of the text the runtime receives:
  - a text `Qz7` followed by the rejected character fails with an error naming the position, the character and the device's reason, and without `Qz7`;
  - the rejected character written as a `\u` escape is reported at its backslash;
  - `Qz7<Kq9>w` names `Kq9`, its position and how to write a literal `<`, without `Qz7`;
  - parse errors give the position and the explanation of design decision 7, and none contains `EOI`, `segment`, `sequence` or the text: `Qz7<Kq9` at 4 and `<Ctrl A` at 1 (unclosed `<`); `Qz7<Kq9 ` at 9 (the end: key block not closed); `Qz7\` at 4 and `ab\`, a line break, `cd` at 3 (escapes nothing); `Qz7<>` at 5 and `a<<b>` at 3 (key name expected, with the `\<` hint); `äöü<Kq9` at 4 and `ab`, a line break, `cd<Kq9` at 6 (non-ASCII text and a second line);
  - `Kw\xQz` and `C:\users` name the invalid escape and its position (3 for `C:\users`), without `Kw`;
  - `format!("{err:?}")` of a parse error does not contain the text;
  - the sensitive rendering of each of these errors gives the position and the kind of failure, with the syntax hint where it applies, and contains no fragment, no rejected character and no device message;
  - a send failure keeps the device's reason, and its sensitive rendering says only that sending failed; the sensitive rendering of a runtime without a platform says that no keyboard device is ready.

  The existing test that matches `Parse(_)` (`keyboard_sequence.rs:330`) follows the new variant shape. Verify all fail today (`crates/runtime/src/keyboard_sequence.rs:16-23`, `:96-110`, `crates/runtime/src/runtime/error.rs:21-27`).
- [ ] 4.4 Add the RF-level tests for the keyword layer. Use a fixture suite under `tests/PlatynUI/robot/`, following the `robot-test-style` skill, run with `python -m robot` in a subprocess and read with `ExecutionResult`, as `test_native_logging_rf.py` does. Assert, against the mock:
  - `Pointer Click` on the OK button at `--loglevel DEBUG` logs one DEBUG line in the form of design decision 6, with `Button "OK"`, the coordinates, the point source and `LEFT`, and there is no such line at `--loglevel INFO`;
  - without `native_log_level`, a run that creates the runtime and clicks contains no native INFO record; with `native_log_level=info`, the runtime's initialization record is at INFO and names the backend `mock` and the provider id `mock`, and the ten `Pointer Click` keywords contain no native INFO record. The fixture creates the runtime with an earlier keyword that needs it, and counts native INFO records only inside the ten `Pointer Click` keywords, because runtime shutdown also logs at INFO;
  - `Keyboard Type` at `native_log_level=debug` and `--loglevel TRACE` logs the length of the text as written, no native `[module]` message or keyword line in `output.xml` contains the text, and no message contains `mock-keyboard: press` or `mock-keyboard: release`. A second run at `native_log_level=trace` shows those records, spelling the typed keys in order, which proves they moved to trace and were not removed;
  - a `Secret` (RF 7.5 is installed) typed with `Keyboard Type` at `--loglevel TRACE` and `native_log_level=debug` passes, its keyword line says `secret` without a length, and the value appears nowhere in `output.xml`. In the trace run, the press records spell the value in order, which proves it was typed;
  - `Keyboard Type` with `Qz7<Kq9>w` fails with a message naming `Kq9`, its position and how to write a literal `<`, including the Robot Framework form `\\<`, without `Qz7`; the same text as a `Secret` fails with the position and the kind of failure, naming neither `Qz7` nor `Kq9`;
  - `pa\<ss>wd` in the fixture, which reaches the keyword as `pa<ss>wd`, fails with a message that mentions `\\<`, and `pa\\<ss>wd` passes;
  - a failing `Get Attribute` assertion without `assertion_message` does not start with `None`;
  - `Pointer Click    /` (the desktop, which has no activatable window) with `auto_activate` on logs a DEBUG line naming the element and the activation error, and the click proceeds;
  - `Set Root` with a query that matches nothing, then `Set Query Settings    {'timeout': 0.5}` and `Wait Until Exists` with a relative target, fails with the root text of design decision 12, including `0.5`, and without `UiNodeDescriptor`; `tests/BareMetal/wait_keywords.robot:28` still passes.

  The fixture creates each `Secret` with `VAR    ${secret: Secret}    %{PLATYNUI_TEST_SECRET}`, because a `Secret` cannot be written as a literal; the pytest sets one variable for the value that types and one for `Qz7<Kq9>w` in the subprocess environment.

  Add pytest cases:
  - BareMetal's description helper, with a stub node that records its `describe()` calls and a stub action that records when it ran: at DEBUG, `describe()` runs before the action; at INFO it is not called;
  - a subprocess (`sys.executable -c …`) that replaces `sys.modules["robot.api.types"]` with a stub module that carries `KeywordArgument` and `KeywordName` but no `Secret` (as on RF < 7.4) before it imports `PlatynUI.BareMetal`, asserts that `get_type_hints(BareMetal.keyboard_type)["text"] is str`, types a plain string on the mock, and checks that a failing plain string still names the key. The test process itself is not changed.

  Verify all fail today, except the older-RF subprocess test, which guards the fallback and passes before and after the change.
- [ ] 4.5 Update `tests/PlatynUI/test_devices.py:330-360` to the logger name `platynui.core.adapter_devices`. Verify it fails until the rename lands.

## 5. The high-priority findings and typed text

Each item names the verified location. Where the path is reachable without a display, its test comes first in the same task or in section 4. Otherwise the task names the real-provider check. New and changed records follow the message style and the field names of design decision 1.

- [ ] 5.1 Configuration (design decision 8):
  - core: `ConfigMap::unknown_keys(known)`, treating `enabled` as known, and `try_bool/try_i64/try_str/try_map -> Result<Option<_>, ConfigTypeMismatch { key, expected, found }>`; core stays log-free;
  - every PlatynUI component checks its own section at the start of its build, before anything can fail. It warns for each unknown key and each type mismatch, naming `component` and `key`, and applies the default. Its known keys are constants that its reads use:
    - X11 (`display`, now read with `try_str`, `crates/platform-linux-x11/src/x11util.rs:63-65`) and Wayland in `crates/platform-linux/src/lib.rs`;
    - the Windows platform, the mock platform, the mock provider (add `tracing` to `crates/provider-mock/Cargo.toml`) and macOS AX (add `tracing` to `crates/provider-macos-ax/Cargo.toml`), which read no keys;
    - AT-SPI (`bus_address`, `surface_popups`), UIA (`honor_window_claims`, `crates/provider-windows-uia/src/provider.rs:370`);
    - Java (`enabled`; `agent` and `jab` through `try_map`), whose backends check their own maps in `AgentBackend::from_config` and `JabProvider::from_config`, reporting the keys as `agent.<key>` and `jab.<key>`;
  - runtime: `platform.backend` is read with `try_str` and warned on mismatch; `select_platform` returns the chosen factory's id (for 5.2); unclaimed component ids stay debug (`crates/runtime/src/runtime/mod.rs:222-235`);
  - binding: `parse_runtime_config` (`packages/native/src/runtime.rs:1418-1502`) warns for a top-level key other than `platform`/`providers`, naming the key and the accepted buckets `platform` and `providers`; for a bucket that is not a dict, naming the bucket and its Python type; and for a non-string key, a leaf or a list element of an unsupported type, naming the dotted path and the Python type.

  Verify:
  - 4.1 and the configuration cases of 4.2 pass;
  - every PlatynUI factory has a unit test that builds it with a bogus key and asserts one warning;
  - the Java and UIA checks run on Windows (6.3).
- [ ] 5.2 Runtime:
  - `desktop.rs:134`: runtime-owned latch per provider — `error!` naming the provider on `Started`, `debug!` on `Continuing`, re-armed on the next Ok, which records one debug naming the provider. Test with a failing fake provider over repeated enumerations: one error, the debug record of the recovery, and after a successful and then a failing enumeration, a second error.
  - `mod.rs:149`: warn when no provider is active.
  - `mod.rs:341`, removing the warn at `:350`: warn naming the candidate backends and what stops working.
  - In a `mock-provider` build of the extension without the mock backend, both records are debug, because 5.9's test-build warning is the one report of that situation. The extension passes this to the runtime as a construction option; `platynui-runtime` does not decide it from its own `mock-provider` feature, so the CLI's and the Inspector's mock builds keep both warnings.
  - `mod.rs:238`: one info record with backend, forced, provider ids, desktop bounds and monitor count, in the form of design decision 1's example. Drop the debug lines at `:149` and `:159`, and log candidates at debug in `select_platform`.
  - The runtime's errors for a missing platform (`crates/runtime/src/runtime/window.rs:129`, `:145`, `:161`, `crates/runtime/src/pointer.rs:257`) say "runtime has no platform backend (none could serve this session, or the runtime is shut down)" instead of naming internal types.
  - `keyboard.rs:43`: debug instead of warn, because the failure is returned, with the number of keys only, and only when keys are still pressed. Add an `error!` with the number of keys still pressed only when `release_all_pressed` fails.
  - `pointer.rs:519`: `warn!` with the requested and clamped coordinates.

  Verify each with a unit test using a scoped subscriber; for the missing-platform errors, that highlight, screenshot and pointer on a runtime without a platform return the new text.
- [ ] 5.3 AT-SPI (design decisions 3 and 12):
  - a provider-owned `Arc<Transitions<String>>` keyed by bus name, and a map from bus name to application name and pid, filled during enumeration (`crates/provider-atspi/src/lib.rs:202-254`, `identity::peer_of`); both are pruned with `retain` against the registry's applications at each enumeration, and passed into `AtspiNode::new` like the popup registry;
  - a latch-aware helper that takes the latch, the bus name, the call name and its timeout (`block_on_timeout_call` passes `TIMEOUT_CALL`). The first timeout of a bus warns with `application`, `pid` (when known), `bus_name`, `call` and `timeout_ms`; later timeouts are debug; a successful call never re-arms. It serves the 26 per-node call sites of `node.rs` (all 28 except the two proxy builds), the per-application reads of the enumeration (`lib.rs:243-254`: child count, interfaces, role, name) and `popup_is_live` (`popups.rs:179`), each keyed by the application's bus name. When the first timeout of an episode makes enumeration skip an application, its warning says that the application's elements are missing from query results;
  - calls without an application subject keep a latch-free helper that logs at debug: the proxy builds (`node.rs:453`, `:511`, `lib.rs:231`), `popups.rs:225`, `identity.rs:259`, the registry calls at `lib.rs:190-204` (`block_on_timeout_init`) and the registry reads of `application_for_pid` (`lib.rs:359-369`);
  - an application instance that `retain` forgets records one debug naming it and its bus name, as the end of its episode;
  - the slow-resolution warning in `get_nodes` (`lib.rs:279-284`) becomes debug with `elapsed_ms` and `bus_name`: a slow call is debug by the level table, and an application that stops answering is reported by the timeout latch;
  - `block_on_timeout` no longer logs for timeouts its callers return (`lib.rs:190-204`, `popups.rs:204-208`, `connection.rs:30-37`, `:54-56`);
  - `connection.rs:32` and `:36` become debug;
  - `extents.rs`'s `Substitutions` moves onto `Transitions`, and its warning's `window` field uses `describe`, replacing the provider's own format (`node.rs:1504-1509`); when the window manager answers again for a failing window, it records one debug naming the window.

  Verify:
  - a unit test with a scoped subscriber and a 10 ms timeout on `std::future::pending()`: ten timed-out calls on bus `:1.7` give exactly one warning naming the application, `bus_name`, `call` and `timeout_ms`, then debug records; neither a call that succeeds in between nor a call that returns an error re-arms; a timed-out call through the latch-free helper gives a debug record only; bus `:1.8` is independent; `retain` forgetting `:1.7` records one debug end of its episode, and its next timeout warns again; the recovered extents window records one debug;
  - a unit test with a scoped subscriber showing no warning on a returned timeout;
  - `just test-crate platynui-provider-atspi` green;
  - after the X11 lane, `robotcode results log --level WARN --execution-messages` shows no timeout warning.
- [ ] 5.4 Typed text (design decision 7):
  - positions: convert pest's byte offsets to character positions; `SequenceSegment::Text` holds `(char, position)` pairs, and shortcut keys keep their positions;
  - errors: give `KeyboardActionError` (`crates/runtime/src/runtime/error.rs:21-27`) the positioned variant with the character or key name, the shortcut flag and the device error, raised while resolving (`keyboard_sequence.rs:99`, `:109`); `ResolvedKeyboardSequence`, `KeyboardEngine::execute` and core's `KeyboardError` stay unchanged;
  - the parse error stores the position and the explanation instead of pest's error; escape errors add the position;
  - the sensitive rendering on `KeyboardActionError`, which gives an error without a position by its kind, without the device's text: `NotReady` and `InputInProgress` keep their fixed texts, and any other device error reads "starting the input failed" when `KeyboardEngine::new` raised it and "sending failed" otherwise;
  - `packages/native/src/runtime.rs:1160-1185` (the three keyboard bindings with their `#[pyo3(signature…)]` attributes) and `:1528-1530` (`map_keyboard_err`): a keyword-only `sensitive: bool = False` that selects the sensitive rendering; update `_native.pyi`;
  - keyboard records that name keys to trace, fields unchanged: `crates/platform-linux-x11/src/keyboard.rs:679`, `:820`, `crates/platform-mock/src/keyboard.rs:115`, `:119`.

  Verify:
  - 4.3 and the keyboard cases of 4.4 pass;
  - a pytest on `Runtime.new_with_mock()`: `keyboard_type('Qz7<Kq9>w')` raises a `KeyboardError` naming `Kq9` without `Qz7`, and `keyboard_type('Qz7<ab')` one naming the position and the unclosed `<` without `Qz7`; with `sensitive=True`, neither contains a fragment;
  - once by hand in the X11 session: a suite that imports BareMetal with `native_log_level=debug` and runs at `--loglevel TRACE` types a `Secret`, read from an environment variable, that contains a character outside the `de` layout (such as `鍵`). `output.xml` contains no `will use dynamic remap` record and not that character, while the runtime's `keyboard execute` record of the same keyword is there.
- [ ] 5.5 Wayland input (`crates/platform-linux-wayland/src/input/mod.rs:105`): the `try_*` helpers (`:173-223`) return their failure. When a later backend succeeds, `initialize` records each rejected one with its reason at debug, and the record of the chosen backend also names the `compositor` type that fixed the order of the attempts; when none succeeds, the final warning names each backend with its reason. Verify with a unit test of both summaries, and in the sidecar harness with a dead control socket and no EIS.
- [ ] 5.6 Java agent (Windows):
  - `crates/provider-java/src/agent/session.rs:213`: an answer with an error resets the failure count and logs debug; only timeouts and transport, protocol and no-agent errors count toward degraded;
  - `agent/backend.rs:177`: version mismatch warned once per pid through the latch, with `retain` against the live handshakes next to `retire_dead_sessions`; a JVM that `retain` forgets records one debug naming its pid.

  Verify with unit tests on Windows (6.3).
- [ ] 5.7 Windows:
  - `crates/provider-java-jab/src/provider.rs:169`: debug when the DLL is missing. Without a client, the JAB backend still lists the visible top-level windows (window enumeration and class names, no bridge call), skips windows that the exclusions assign to a stronger backend, and reports its `SunAwt*` windows as unserved with the cause "Access Bridge DLL not found" and their pids among the Java processes, so that automatic attachment can reach those JVMs. After the re-sweep, the router's `emit_enablement_diagnostics` (`crates/provider-java/src/provider.rs:315`) warns once per process for that cause, naming `providers.java.jab.dll_path`, `PLATYNUI_JAB_DLL` and a 64-bit JDK, instead of its `jabswitch` hint;
  - `crates/java-agent/src/attach/windows.rs:170`: leak the stub allocation when the remote thread has not finished, log debug, and fix the Drop comment (`:291-293`);
  - `crates/provider-windows-uia/src/node.rs:596`: after activation, debug when the window did not become the foreground window (decision 10), plus a debug at entry.

  Verify with `just clippy-windows` and `cargo clippy --target <windows target> -p platynui-java-agent`. Behavior on Windows (6.3), including an agent-served Swing application without the DLL, which logs no warning about the bridge.
- [ ] 5.8 Double reports (design decision 12):
  - `crates/platform-windows/src/pointer.rs:98-99`: read the error first, then log it at debug, then return it (`let err = last_error("SendInput"); tracing::debug!(error = %err, …); return Err(err);`), so that no subscriber runs between `SendInput` and `GetLastError`;
  - `crates/platform-windows/src/screenshot.rs:89-96`: put the `windows::core::Error` into the returned details and log it at debug after building the error;
  - `crates/platform-linux-x11/src/screenshot.rs:41-42`: debug.

  Verify with `just clippy-windows`, and a unit test or review that each failure is still returned with its code.
- [ ] 5.9 macOS, mock and test builds:
  - `crates/provider-macos-ax/src/lib.rs:29`: warn once per process;
  - `crates/platform-mock/src/pointer.rs:58`: `trace!` per move;
  - `packages/native/src/runtime.rs:769`: in a `mock-provider` build, warn once per process when a runtime is created without the mock backend; that construction records no other warning (5.2).

  Verify:
  - a unit test in `crates/provider-macos-ax`, which builds on Linux: two provider creations give one warning; and `just check-macos-arm`;
  - a pytest that runs in a subprocess: two `Runtime()` constructions in the mock build log exactly one warning in total, the test-build one, which has reached Python `logging` when the first constructor returns (3.3), and `Runtime.new_with_mock()` logs none;
  - the mock lane unchanged.
- [ ] 5.10 CLI (`crates/cli/src/commands/watch.rs:46`): warn when no active provider has event capabilities, otherwise debug. Verify with a unit test of the provider check (no event providers gives the warning) and a CLI test against the mock (which has events) with no warning.
- [ ] 5.11 Extension profile extractors (`packages/native/src/runtime.rs:2819`, `:2953`, `:3029`, `:3171`, `:3284`): `TypeError` for a wrong type, warn for an unknown key. Rebuild the mock module. Verify the profile cases of 4.2 pass.
- [ ] 5.12 Python library:
  - rename the logger (4.5); BareMetal logs through `logging.getLogger("platynui.baremetal")`, and `robot.api.logger` stays only for the screenshot embedding (`src/PlatynUI/BareMetal/__init__.py:2375`, `:2393`); `:2441` (Highlight skips an element whose bounds cannot be read) logs through `platynui.baremetal` at debug instead of `logger.trace`;
  - add `_log_action(verb, descriptions, **details)`, which takes descriptions, never nodes; each action keyword of design decision 6 builds them before it acts, only when the logger is enabled for DEBUG, and logs the line in decision 6's form after success; `_resolve_screen_point` returns the point source;
  - `Secret` support in `Keyboard Type`, `Keyboard Press` and `Keyboard Release` with the conditional import and the `KeyboardText` annotation of design decision 7, passing a `str` with `sensitive=False` and a `Secret`'s `.value` with `sensitive=True`; for a plain `str`, an error that carries the `\<` hint adds the Robot Framework form `\\<` (and `\\\\` for a backslash);
  - `_maybe_bring_to_front` logs debug instead of `pass` (`:1649`), describing the element only when DEBUG is enabled;
  - the not-found errors (design decision 12): the target text stays; a root that is not found raises its own subclass with the root text, which `Wait Until Exists` (`:1422`) does not re-wrap; `:279` and `:288` name no internal types; errors chain with `from exc`;
  - `src/PlatynUI/_assertable.py:121`: pass `assertion_message or ''`, so a failure without a message no longer starts with `None`.

  Document in the keyword docs, in the user-facing voice: the DEBUG lines; `Secret` support; the sequence syntax, a `Secret`'s value included (`\\` types a backslash; `\<` and `\>` type `<` and `>`; `\xHH` and `\uHHHH` type the character with that code, and are an error when not followed by 2 or 4 hex digits; a backslash before any other character is dropped); that Robot Framework test data doubles each backslash (`\\<`), linking `dev-docs/keyboard-input.md` §6; and that a `Secret` must be the whole argument, with the two ways around it from design decision 7. Verify:
  - 4.4 and 4.5 pass under `just test-python`;
  - `just test-baremetal` stays green;
  - `just mypy` is clean.
- [ ] 5.13 User-facing documentation:
  - the BareMetal library introduction (`src/PlatynUI/BareMetal/__init__.py:938`, `:953-955`, `:960-973`): the level names (`off`, `error`, `warn`/`warning`, `info`, `debug`, `trace`, `critical`, `fatal`); that a single level raises only PlatynUI's own detail, while `error` and `off` apply to every module, and `RUST_LOG` is the way into third-party modules and the only source of filter directives; that an unknown setting key of the active backend or provider and a misspelled top-level key are reported as warnings, while blocks for other platforms stay silent; that a profile value of the wrong type fails;
  - a short "Reporting a problem" paragraph there: `native_log_level=debug` with `--loglevel DEBUG`, attach `output.xml`; `trace` names every typed key, a `Secret`'s included, so a trace log of a run that types secrets is not for sharing; the same caveat in `Keyboard Type`'s documentation;
  - the `Runtime` docstring (`packages/native/src/runtime.rs:760-763`) and its stub, and `src/PlatynUI/core/adapter_devices.py:45`.

  Verify that `grep -rn "platynui\.devices\|tolerated (ignored)\|simply ignores the ids and keys" src packages dev-docs --exclude=logging.md` prints nothing (`dev-docs/logging.md` names the old logger for migration), and read the rendered libdoc of `PlatynUI.BareMetal`.

## 6. Verification

- [ ] 6.1 Run `just check`, `just test`, `just test-python`, `just cross-target-checks`, then `just build-native`. Verify everything is green.
- [ ] 6.2 Run `just test-baremetal`, `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor`. After each lane, run `just test-summary` and `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - all lanes are green;
  - no warning or error comes from PlatynUI;
  - any entry is fixed in this change or recorded as a follow-up;
  - in the X11 session, `Runtime(config={'platform': {'x11': {'dispaly': ':1'}}})` constructs, uses the environment's display, and logs one warning naming `platform.x11` and `dispaly`.
- [ ] 6.3 Before archiving, check what needs other machines, and record the outcome in this task:
  - Windows: `just test` (the Java and UIA parts of 5.1, 5.6, 5.7, 5.8), then `just test-acceptance-windows` and its warnings through `robotcode results log --level WARN --execution-messages`, including the JAB scenarios of 5.7 when the machine lacks the bridge;
  - macOS: `just check-macos-arm` (5.9).

  If no Windows machine is available, move 5.6, 5.7, 5.8 and the Windows parts of 5.1 into a follow-up change before archiving, instead of archiving them unverified. Move with them the spec text they implement: the diagnostic-logging scenarios *A string where a flag was expected, Java* and *Java Access Bridge is missing only when it matters*, the Java Access Bridge bullet of *A capability that is not there says so once*, the foreground-window bullet of *Fallbacks that change an action's effect*, and the `jab-provider` and `java-app-classification` deltas; and the matching bullets of the proposal and of design decisions 10 and 12, so that archiving adds no unimplemented behavior to the main specs.

## 7. Commit (only when the user asks)

- [ ] 7.1 Commit in reviewable steps, each lint-clean on its own:
  - concept and rules;
  - core building blocks and `platynui-log-filter`;
  - extension and binaries;
  - one commit per area's fixes under that area's scope;
  - the Python library and its documentation.

  Subjects ≤ 72 characters. Commits that change behavior name the change in their body, for the changelog: the filter commit (a single level no longer lowers third-party crates, `RUST_LOG` does; `error` and `off` apply to every module; `PLATYNUI_LOG_LEVEL` takes a single level only), the configuration commit (unknown top-level keys and wrongly typed settings are warned about), the profile commit (a wrongly typed value raises `TypeError`) the logger rename (`platynui.devices` → `platynui.core.adapter_devices`), the keyboard commit (errors give the position and no longer repeat the input; records that name keys move from debug to trace), and the not-found commit (a root set by `Set Root` that is not found raises its own subclass with a new text). Whether the profile change carries `BREAKING CHANGE:` is the maintainer's call.
