# Design

## Context

See proposal.md for the motivation and the specs for the required behavior. The facts below come from two sources:

- the project-wide logging review behind this change: seven area surveys plus a conventions survey, each proposal checked against the code by a separate verifier;
- a four-lens review of this change's own draft: references, consistency, test feasibility and technical soundness.

*Verified* marks what was read in the code. *Inferred* marks conclusions. Robot Framework line numbers refer to RF 7.5, the installed version.

**What the rules say today (verified).**

- `.github/instructions/tracing.instructions.md` is the only normative logging document, and it covers Rust only.
  - It sets out the level table (`:35-41`), never `error!` for a returned `Err` (`:45`), structured fields (`:120-132`), and the precedence `RUST_LOG` > `--log-level` > `PLATYNUI_LOG_LEVEL` > `warn` (`:61-66`).
  - Its error examples (`:37`) are all failures that are returned as `Err`: `crates/platform-windows/src/pointer.rs:98-99`, `crates/platform-windows/src/screenshot.rs:89-90` and `crates/platform-linux-x11/src/screenshot.rs:41-42`.
- `dev-docs/error-handling.md:165-176` keeps `platynui-core` free of `tracing`.
- Python has no rules. `dev-docs/python-library-design.md` §A.9.7 (`:3525-3532`) wrongly says users must call `setLevel(DEBUG)`. Under Robot Framework, the root logger follows `--loglevel` (`robot/output/pyloggingconf.py:52-57`).
- The old rule file is linked from four places:
  - `.github/copilot-instructions.md:117-118`, which still says only the binaries install a subscriber;
  - `dev-docs/architecture.md:768`;
  - `dev-docs/python-bindings.md:71`;
  - the `native-log-bridge` change.

**The native log bridge (verified).**

- Native records reach Python `logging` under `platynui.native.<module>` and the Robot Framework log in the keyword they belong to. A native `WARN` is an RF `WARN` (`packages/native/src/log_bridge.rs`, `src/PlatynUI/_our_libcore.py`).
- The Python library validates `native_log_level` against `error`…`trace` before the extension sees it (`src/PlatynUI/core/native_logging.py:20-27`) and ranks requests by that tuple (`:72`).
- The `native-logging` main spec, archived with `native-log-bridge`, fixes these values.
- `PLATYNUI_LOG_LEVEL` is used verbatim, so `PLATYNUI_LOG_LEVEL=debug` opens third-party crates (`log_bridge.rs:210-215`).
- Records are delivered when a runtime call returns through `RuntimeAccess`. The constructors do not go through it (`packages/native/src/runtime.rs:762-768`).

**Subscribers (verified).**

- The two shipped binaries install an `fmt` subscriber, each from its own copy: the CLI (`crates/cli/src/lib.rs:44-74`) and the Inspector (`apps/inspector/src/lib.rs:559-579`). The development tools (the Wayland compositor, the EIS test client, the egui test app) have copies of their own and are outside this change.
- Their `--log-level` is a case-sensitive clap `ValueEnum` without `warning`.
- The CLI help names a variable that does not exist, `PLATYNUI_LOG` (`crates/cli/src/lib.rs:80`).
- `EnvFilter` splits a spec on `,`. A single word that is not a level becomes a target directive at TRACE (`tracing-subscriber 0.3.23 src/filter/env/directive.rs:213-221`), and `tracing-core`'s `LevelFilter::from_str` knows no `warning` (`metadata.rs:799-805`). So `PLATYNUI_LOG_LEVEL=WARNING` silently switches everything else off.
- A target-only `RUST_LOG=zbus` is standard syntax for "every level of zbus".
- The Inspector depends on crates that log through `log` (its whole render and window stack), and nothing forwards those records.
- With its `tracing-log` feature, tracing-subscriber's `try_init` installs `LogTracer` itself (`src/util.rs:62-73`).

**Configuration (verified).**

- `ConfigMap::get_*` return `None` for a value of the wrong type (`crates/core/src/config.rs:104-140`).
- Components read their keys ad hoc:
  - X11 `display` (`crates/platform-linux-x11/src/x11util.rs:64`);
  - AT-SPI `bus_address` and `surface_popups` (`crates/provider-atspi/src/lib.rs:72-75`);
  - UIA `honor_window_claims` (`crates/provider-windows-uia/src/provider.rs:370`);
  - Java top-level `enabled` (`crates/provider-java/src/provider.rs:48,81`), its `agent` map (`agent/backend.rs:101-106`, including `call_timeout_ms`) and its `jab` map (`crates/provider-java-jab/src/provider.rs:122-125`, a library behind the Java factory, not a factory itself).
- The X11 and Wayland `PlatformFactory` implementations live in `crates/platform-linux/src/lib.rs:26-70`.
- The whole Java provider is compiled on Windows only (`crates/provider-java/src/lib.rs:28-39`).
- The runtime logs unclaimed component ids at debug (`crates/runtime/src/runtime/mod.rs:222-235`). `select_platform` calls `create` only for the forced factory or the first one whose `can_serve` accepts, and returns the bundle but not which factory built it (`:305-342`).
- `configured_display` and `platform_backend` read with `get_str`, so a value of the wrong type silently falls back to the environment or to auto-detection (`crates/platform-linux-x11/src/x11util.rs:63-65`, `crates/core/src/config.rs:188-190`).
- The Java backends receive their own sub-maps (`AgentBackend::from_config`, `JabProvider::from_config`).
- `parse_runtime_config` reads only `platform` and `providers` and drops any other top-level key without a record. It drops a bucket that is not a dict, a non-string key and a leaf of an unsupported type at debug (`packages/native/src/runtime.rs:1418-1456`), before any component can see them.
- The five profile extractors drop a wrongly typed value (`packages/native/src/runtime.rs:2819,2953,3029,3171,3284`), while sibling fields raise.

**Python and typing (verified).**

- Loggers are `platynui.devices` (`src/PlatynUI/core/adapter_devices.py:30`, tested at `tests/PlatynUI/test_devices.py:330-360`) and `platynui.ui.application`.
- `_maybe_bring_to_front` swallows every failure with `pass` (`src/PlatynUI/BareMetal/__init__.py:1649`).
- Not-found errors name `UiNodeDescriptor` (`:279`, `:288`). `Wait Until Exists` re-wraps a `Set Root` failure as a failure of its target (`:1422`).
- Robot Framework records every keyword's arguments in `output.xml` as written in the test data (`robot/running/librarykeywordrunner.py:67-74`, `robot/output/xmllogger.py:74`): a literal verbatim, a variable by its name. At TRACE it also logs the resolved values (`librarykeywordrunner.py:86-88`, `:106-111`), where only a `Secret` is hidden. `robot.api.types.Secret` exists since RF 7.4. A `Secret` embedded in a longer argument is replaced by the text `<secret>` (`robot/variables/replacer.py:111`). The project requires `robotframework>=7.0.0` (`pyproject.toml:31`).
- RF calls `str()` on every assigned return value (`robot/variables/assigner.py:126`). An element repr that asked the provider would cost D-Bus calls on every `Query` assignment. On AT-SPI, `id()` is uncached and costs up to two calls with a 1 s timeout each (`crates/provider-atspi/src/node.rs:709-716`).
- The mock nodes have no id (`crates/provider-mock/src/node.rs`, `assets/mock_tree.xml:69`).
- Under Robot Framework a `logging` record from the keyword thread reaches the RF log like `robot.api.logger`, and RF keeps the root logger's level in sync with `--loglevel` and `Set Log Level` (`robot/output/pyloggingconf.py:52-57`). Outside RF, `robot.api.logger` writes to a logger named `RobotFramework` (`robot/api/logger.py:130-133`). Robot Framework does not capture Python warnings.
- `_assertable.py:121` passes `assertion_message` as AssertionEngine's `message` prefix, so a failure without a message starts with the literal text `None` (reproduced on the mock).

**Typed text (verified).**

- **Only the runtime knows positions.** `KeyboardSequence::resolve` is the only production caller of `key_to_code`, once per text character (`crates/runtime/src/keyboard_sequence.rs:96-100`) and once per shortcut key (`:105-110`). A device sees one character or key name at a time. All three keyboard entry points go through parse and resolve (`crates/runtime/src/runtime/input.rs:320-321`, `:341-342`, `:362-363`).
- **The parse error repeats the whole input.** It wraps a pest error whose display prints the input line with a caret (`keyboard_sequence.rs:16-17`).
- **Device errors say what, but not where.** Every backend names the character or key it cannot convert in `UnsupportedKey`: X11, the three Wayland backends, Windows and the mock. The runtime passes the error on with `?` and adds no position. The grammar reads any unescaped `<word>` as a shortcut, so a text such as `pa<ss>wd` fails with `unsupported key: ss` and no hint that `\<` types a literal `<`.
- **The binding passes the display on.** `map_keyboard_err` raises it as the Python `KeyboardError` (`packages/native/src/runtime.rs:1528-1530`). The keyboard bindings take the sequence as a plain `str` (`packages/native/python/platynui_native/_native.pyi:432-434`).
- **Conversions fail at resolve time.** Every backend rejects a character or key it cannot convert in `key_to_code`; X11 does so too when no spare keycode is left for a remap (`crates/platform-linux-x11/src/keyboard.rs:677-682`, `:817-823`). Send-time errors are device or I/O failures, such as `NotReady` or "foreign key code".
- **Robot Framework unescapes arguments first.** `pa\<ss>wd` in a `.robot` file reaches the keyword as `pa<ss>wd`; test data needs `\\<` (`dev-docs/keyboard-input.md:124-129`).
- **The parse error cannot say why by itself.** With this grammar and pest 2.9.1, pest's variant names rules, not the missing `>`: `Qz7<Kq9` and a trailing `\` both give "expected EOI or segment", `<Ctrl A` gives "expected sequence". Its location is a byte offset, and `line_col` restarts on every line. The error keeps the input line, which its derived `Debug` prints.
- **Positions are lost when parsing.** `SequenceSegment::Text` keeps only the decoded text (`keyboard_sequence.rs:36`), and `KeyboardActionError` passes the core `KeyboardError` through transparently (`crates/runtime/src/runtime/error.rs:21-27`).
- **Per-character records sit at debug.** X11 records a shortcut key's name (`crates/platform-linux-x11/src/keyboard.rs:679`) or a character (`:820`) with its keysym at debug when it needs a dynamic remap. Its other per-key records are already at trace. The mock records every key name at debug (`crates/platform-mock/src/keyboard.rs:115`, `:119`).
- **The Python layer.** The keyboard keywords accept `str` only (`src/PlatynUI/BareMetal/__init__.py:2217`, `:2259`, `:2292`). BareMetal evaluates annotations at definition time, and robotlibcore reads them with `get_type_hints`. A `Secret` is not a `str` subclass. It cannot be written as a literal in test data: it comes from `VAR    ${pw: Secret}    %{ENV}` or `--variable 'NAME: Secret:value'`.

**AT-SPI connection ownership (verified).**

- Nodes hold the atspi crate's `Arc<AccessibilityConnection>` (`crates/provider-atspi/src/node.rs:50`).
- Most per-node reads go through free helpers that receive only the connection and object, or only a future. There are 28 `block_on_timeout_call` call sites in `node.rs`, plus sites in `lib.rs`, `identity.rs` and `popups.rs`. Two of them are proxy builds (the `make_proxy!` builders at `node.rs:453`, and the application proxy at `:511`), whose `build()` with `CacheProperties::No` finishes without a bus round trip and always succeeds. Enumeration reads each application's child count, interfaces, role and name (`lib.rs:243-254`) on every poll, and the popup watcher probes liveness per popup (`popups.rs:179`).
- Each call gets its own budget of 1 s (`crates/provider-atspi/src/timeout.rs`), so one slow handler of an application times out while its next call answers.
- The popup registry and the injected window manager show how provider-owned state is passed to every node (`node.rs:84`, `extents.rs:31-43`).
- The connection is opened lazily inside `get_nodes` (`lib.rs:122-129`), and its failures are logged with `error!` before they are returned (`connection.rs:30-37`, `:54-56`).

## Goals / Non-Goals

**Goals:**

- One written concept for Rust and Python, applicable without reading this change.
- The building blocks every later fix needs: a transition latch, a node description, one filter builder.
- The high-priority findings of the library fixed, adjusted to the maintainer's decisions and to what this review found. That includes the double reports in the old rule document's own examples.
- Healthy lanes free of PlatynUI warnings.
- Keywords that do not repeat the text they type, errors that say why typing failed, and `Secret` support (decision 7).

**Non-Goals:**

- The development tools (proposal, *Scope*).
- The review's other findings. They are persisted in `review-findings.md` next to this design, per area and with the verifier's verdict, and follow as one change per area.
- The sweep of existing field names and message wording to the new style. This change applies the style to what it touches.
- INFO-level keyword narration (maintainer decision: DEBUG).
- Tooling that reports or fails on lane warnings (maintainer decision, decision 11).
- Attributing native records to a runtime or library instance.

## Decisions

### 1. The concept lives in `dev-docs/logging.md`; the rule file points to it

`dev-docs/logging.md` is the normative concept, written as explanatory prose with examples. It covers:

- the channels: native diagnostics, and the Python library's own records including the keyword lines;
- diagnostics inside a target JVM, in one paragraph: the Java agent writes to the target's stderr, errors always and debug only with `-Dplatynui.agent.debug=true`; the level knob and the Robot Framework log do not reach it; the Rust side reports what it observes, such as a missing handshake, as a warning that names that log. The details stay in `dev-docs/java-toolkits.md` and `AgentLog`, which it links;
- the level table (proposal, *The levels*) and who sees each level;
- log-or-return;
- once per episode, and how an episode ends;
- decisions;
- external boundaries;
- the message style and the field names below;
- producer rules: a native component logs under a target that starts with `platynui`, or the level knob does not reach it; diagnostics are events and carry their context in fields, because spans are not carried to Python or Robot Framework; expensive message parts are built only when their level is enabled;
- the node description;
- the Python mechanisms and logger names;
- typed text and `Secret` (decision 7);
- the level knob;
- how to look at a run's warnings (decision 11).

It states rules, not the review's counts.

**Message style and fields.** Messages are English fragments in lower case; proper names such as AT-SPI, X11 and JAB keep their case. They have no trailing period and no type, function or module prefix (the target names the module). Native messages carry no interpolated values; their values go into fields. Python records, which Robot Framework shows as text only, carry their values in the message through lazy `%` arguments, as the keyword lines of decision 6 do. A warning or error states its consequence after a semicolon: "provider failed to list its top-level elements; its elements are missing from query results". These field names are fixed now, because this change's records use them:

- `error`, always written `error = %err`, never `err` or `e`;
- `provider`, `component` (bucket form, `providers.atspi`), `key` (dotted, relative to the component), `expected` and `found`;
- `application`, `bus_name`, `pid`;
- `call`, `elapsed_ms`, `timeout_ms`;
- `element` (decision 4);
- `requested_x` and `requested_y` with `x` and `y`;
- `position` (1-based);
- `backend`, `forced`, `providers`, `suppressed`, `desktop` and `monitors` for the runtime's initialization record;
- `window` for a window, in the description form.

The runtime's info record is the worked example: `info!(backend = "x11", forced = false, providers = ?ids, desktop = %bounds, monitors = 2, "runtime initialized")`. The full glossary lives in `dev-docs/logging.md`.

**The rule file.** `.github/instructions/tracing.instructions.md` becomes `.github/instructions/logging.instructions.md`, with `applyTo: '**/*.rs,**/*.py'`: a checklist that states the non-negotiable rules in one line each and links the concept. Instruction files are loaded into an agent's context by `applyTo`, and a link may not be followed, so the checklist keeps the rules themselves; the explanations and the per-layer guidance move to the concept. Two full normative copies of the same rules produced the contradictions this change fixes.

The three live references are updated (copilot-instructions, `architecture.md:768`, `python-bindings.md:71`); the archived `native-log-bridge` change keeps its link. §A.9.7 becomes a pointer with an English summary. `dev-docs/plan-waylandCompositor.md:1193` is corrected.

*Alternative rejected:* extending the Rust-only rule file in place. Its `applyTo` and purpose do not fit, and a concept needs room for explanation.

### 2. Log or return

The level table is the proposal's *The levels*; it is normative in the spec's *Each level has one meaning*. This decision adds how a failure travels between layers.

A layer that returns a failure logs it at most at debug. The layer that swallows it or decides its consequence logs it above debug, and only that layer.

A provider failure swallowed during desktop enumeration is therefore reported once, by the enumeration's latched error (decision 3). The provider's own connection code only returns it.

### 3. A transition latch in `platynui-core`, without `tracing`

`platynui_core::diagnostics::Transitions<K>` records which subjects are currently failing. It offers:

- `failed(key) -> Episode` (`Started` / `Continuing`);
- `recovered(&key) -> bool`;
- `retain(|key| …)`, so an owner can forget subjects that no longer exist.

`failed` and `recovered` accept a borrowed key (`&Q where K: Borrow<Q>`), so a `String`-keyed latch takes `&str` without an allocation. It holds a mutex-protected set, returns facts, and logs nothing. The call site logs warn or error on `Started` and debug on `Continuing`. The end of an episode is recorded once at debug, naming the subject; it is not a lifecycle transition, and never info or warn.

The owner of each latch sets its scope, including what counts as recovery:

| Use | Owner | Episode |
|---|---|---|
| Provider failures during enumeration (`crates/runtime/src/runtime/desktop.rs:134`) | The runtime; an `Arc` shared by the desktop node and its children iterator | Ends when the provider lists its elements again |
| AT-SPI per-node timeouts | The provider: an `Arc<Transitions<String>>` keyed by bus name, passed into `AtspiNode::new` like the popup registry and handed to the per-node read helper | One application instance. A successful call does not end it: an application's calls have independent budgets, so re-arming on success would warn on every poll. D-Bus unique names are not reused, so `retain` against the registry's applications at each enumeration forgets an instance that has gone, and a restarted application warns again |
| Java agent version mismatch | The agent backend | One JVM process; `retain` with the live handshake set next to `retire_dead_sessions` |
| Bounds substitutions (`crates/provider-atspi/src/extents.rs`) | Moves onto `Transitions`, still owned by the injected window manager | Ends when the window manager answers again |

Next to its latch, the AT-SPI provider keeps the application name and pid it reads for each bus name during enumeration (`crates/provider-atspi/src/lib.rs:202-254`), pruned by the same `retain`, so that its first-timeout warning names the application and not only `:1.57`. The per-application reads during enumeration and the popup liveness probe share the per-bus latch. Calls without an application subject use a latch-free helper that records at debug: the proxy builds, the popup watcher's own connection, the bus daemon's identity query and the registry calls. An owner records the end of an episode for each subject that `retain` forgets or that recovers.

Reports that happen once per process rather than per subject use a process-wide `Once` instead: the macOS stub warning, the test-build warning, and each rejected (variable, value) pair of the level knob (decision 5). The missing JAB DLL is reported once per process by the Java provider's router (decision 12).

*Alternatives rejected:*

- A logging helper crate with `tracing`: it pulls `tracing` into every user and duplicates the level choice.
- `tracing` in core: it gives up the rule in `error-handling.md` for one helper.

### 4. One element description, produced on request

`platynui_core::ui::describe(&dyn UiNode) -> String` returns:

- `Role "Name"`, with the role's local name (`Button`);
- plus ` #Id` when `UiNode::id()` is set and not empty after trimming;
- with the name cut at 60 characters: a longer name keeps its first 59 characters, cut at a character boundary, followed by `…`;
- with name and id escaped like Rust's `char::escape_debug` (quotes, backslashes and control characters such as a line break), so that a description is always one line. The name is cut first, counting its own characters, and escaped afterwards, so that an escape is never split.

Where it is used:

- **Python:** the extension exposes it as the explicit method `UiNode.describe()`.
- **Keyword lines and error messages:** BareMetal calls that method.
- **Rust:** new and changed native diagnostics use it for an element field. The AT-SPI extents warning, which this change touches, uses it for its `window` field instead of its own format (`crates/provider-atspi/src/node.rs:1504-1509`).

`UiNode.__repr__` and `__str__` stay free of provider calls. They show the runtime id, because Robot Framework converts every assigned value to text and would otherwise pay D-Bus calls per `Query` result.

*Alternative rejected:* `__repr__` as the description. It is cheap on the mock and on UIA, where the id is cached, but costs up to two D-Bus calls per element on AT-SPI.

### 5. One filter builder for all entry points: `crates/log-filter`

A small new crate, `platynui-log-filter`, takes over `filter_spec` and its tests from `packages/native/src/log_bridge.rs`.

`parse_level(&str) -> Result<LevelFilter, UnknownLevel>` is case-insensitive. It accepts `off`, `error`, `warn`, `info`, `debug` and `trace`, the Python spelling `warning` (WARN), and `critical` and `fatal` (ERROR). `UnknownLevel` implements `Error` and `Display` ("unknown log level 'verbose'; expected one of off, error, warn (warning), info, debug, trace, critical, fatal"), so the function serves directly as a clap `value_parser`, and the extension turns it into a `ValueError`.

A single level L becomes a filter this way (maintainer decision): for `warn`, `info`, `debug` and `trace` it is `warn,platynui=L`, so the knob never makes a third-party module more verbose; for `error` and `off` it is plain `L`, so these silence every module. `RUST_LOG` is the only way into third-party modules.

`filter_spec(env, requested)` builds the filter from these sources, the first one present winning:

| Source | Resulting filter |
|---|---|
| `RUST_LOG` | Used verbatim when it parses. Target-only directives stay valid. |
| The requested level | L as above |
| `PLATYNUI_LOG_LEVEL`, a single level (aliases included) | L as above |
| none | `warn` |

A rejected value counts as absent, so the next source applies: a `RUST_LOG` that does not parse, and a `PLATYNUI_LOG_LEVEL` that is not a level. An empty or blank variable counts as absent without a report. Directive syntax in `PLATYNUI_LOG_LEVEL` is rejected with the hint to use `RUST_LOG`: directives there duplicated `RUST_LOG`, could open third-party modules, and without a bare level switched off every other target's warnings. Each rejected (variable, value) pair is reported once per process, because the filter is rebuilt whenever a library instance requests or withdraws a level.

The crate has two features:

- `fmt` provides `init_stderr(requested)` for the binaries. It uses `try_init`, and reports rejected values and an initialization failure instead of panicking.
- `log` enables tracing-subscriber's `tracing-log` feature. `try_init` then installs `LogTracer` with the right max-level hint, so the code never calls `LogTracer::init` by hand.

Only the Inspector enables `log`. Workspace builds unify features, so a workspace build of the CLI forwards `log` records too. That only affects which crates' records it shows, and is accepted.

Who uses it:

- The extension depends on it without `fmt`.
- The CLI and the Inspector replace their copies with `init_stderr`. Each `--log-level` becomes an optional `value_parser = platynui_log_filter::parse_level` argument, which gives case-insensitivity, the aliases and one rejection message.
- The Python library normalizes `native_log_level` the same way before ranking.

The crate is added to AGENTS.md's crate list and to the justfile's cross-target package lists.

*Alternative rejected:* fixing the copies separately. They already differ.

The development tools may adopt the crate later; this change does not touch them.

### 6. Keyword action lines at DEBUG

BareMetal logs its own records through `logging.getLogger("platynui.baremetal")` (decision 9). A keyword that acts builds the descriptions of its elements before it acts, where it resolves the element and its point anyway, and only when that logger is enabled for DEBUG. Robot Framework keeps the root logger's level in sync with `--loglevel` and `Set Log Level`, so at the default INFO a keyword line costs no provider call, and no keyword queries an element after an action that may have removed it (`Close Window`, a click that closes a dialog). After success, `_log_action(verb, descriptions, **details)` writes one line, after `flush_native_logs()`, so that native records of the same action come first.

The action keywords are Pointer Click, Pointer Multi Click, Pointer Press, Pointer Release, Pointer Move To, Pointer Scroll, Focus, Activate Window, Restore Window, Maximize Window, Minimize Window, Close Window, Move Window, Resize Window, Move And Resize Window, Bring To Front, Keyboard Type, Keyboard Press, Keyboard Release and Highlight. Query, the Wait Until keywords, the Get and Set keywords and Take Screenshot write no action line.

A line has the form `<verb> <what> at <point>, <source>; <parameters>`, for example `clicked Button "OK" #ok at (412, 305), its activation point; button LEFT, 1 click`. A keyword without an element names the point and its source instead (absolute coordinates or the current pointer position). A keyboard keyword gives the length of the sequence as written, or `secret` for a `Secret` (decision 7), and says that it typed into the focused element when it was given none, without querying it. `Highlight` names each element, or the rectangles. `_resolve_screen_point` also returns where the point came from.

How long the resolution waited is not part of the line. The Python-library follow-up records it once, inside the resolution (`review-findings.md`, `src/PlatynUI/BareMetal/__init__.py:273`).

Maintainer decision: DEBUG only.

### 7. Keywords do not repeat what they type; `Secret` is supported

Maintainer decision: PlatynUI is a test automation system. Values read from the application, and how characters were converted into keys, may be reported. A keyword does not repeat the text it was given, because Robot Framework already records its arguments. When typing fails, the error says why, not what was typed.

**Positions travel with the sequence.** Only the runtime iterates the sequence, so it owns positions.

- A position is the character in the text the runtime receives, counted from 1. Under Robot Framework that is the text after RF has resolved its own escapes and variables, not the `.robot` source. pest reports byte offsets; the parser converts them with `input[..offset].chars().count() + 1`, for the parse error as well as for text, key and escape spans.
- `SequenceSegment::Text` holds `(char, position)` pairs, and shortcut keys keep their positions. Positions are attached where the sequence is resolved: every backend rejects a character or key it cannot convert in `key_to_code`, so `ResolvedKeyboardSequence`, the engine and `KeyboardEngine::execute` stay unchanged. An escape such as `\u00E4` or `\xE4` maps to the position of its backslash.
- `KeyboardActionError` (`crates/runtime/src/runtime/error.rs`) gains a positioned variant: the position, the character or key name as parsed, whether it came from a shortcut, and the device's `KeyboardError`. `resolve` raises it (`keyboard_sequence.rs:96-110`).
- A failure while sending carries no position, like one of `start_input`, `end_input` or the cleanup release: send-time errors are device or I/O failures that do not depend on which key was being sent. Core's `KeyboardError` and the devices stay unchanged.

**Errors say why.**

- The positioned error names the character or key itself, from the parsed sequence, and appends the device's message: "the character 'ä' at position 5 cannot be typed: unsupported key: ä". For a key from a shortcut, it adds that a literal `<` is written `\<`. The runtime's hint stays neutral; for a plain string, BareMetal adds that Robot Framework test data needs `\\<` (and `\\\\` for a backslash), because Robot Framework removes a backslash before an unknown character before the keyword runs.
- The parse error stores only the converted position and an explanation, not pest's error, so neither its `Display` nor its `Debug` carries the input. The explanation comes from pest's variant together with the character at the position, never from rule names:
  - when the expected rules contain `key`, a key name is expected there, and a literal `<` inside a key block is written `\<`. At the end of the input, it adds that the key block is not closed with `>`;
  - otherwise, when the character is `<`, this `<` opens a key block that is not closed with `>`, and `\<` types a literal `<`;
  - otherwise, when the character is `\`, this `\` escapes nothing, because it ends the text or comes before a line break, and `\\` types a backslash.
- An escape error keeps its literal, which is the reason, and adds the position.

**Records below the error.** Neither the runtime nor a device can tell whether a sequence is a `Secret`. A keyboard record that names a key, character, key code or keysym, or that carries a device's error text, is therefore trace-level, by the typed-text rule that takes precedence over the level meanings (proposal, *The levels*; spec, *Each level has one meaning*). Below trace, keyboard records carry counts, modes and positions. The X11 remap records (`keyboard.rs:679`, `:820`) and the mock's press and release records move to trace with their fields unchanged. The concept states that trace shows individual keys, including those of a `Secret`.

**`Secret`:**

- The import is conditional, so that RF sees the right annotation and mypy stays clean:

  ```python
  if TYPE_CHECKING:
      from robot.api.types import Secret
      KeyboardText: TypeAlias = str | Secret
  else:
      try:
          from robot.api.types import Secret
      except ImportError:
          KeyboardText = str
      else:
          KeyboardText = str | Secret
  ```

  The keyboard keywords annotate their text as `KeyboardText`. `Secret` is never bound to `str` or `None`.
- A keyword passes a `str` with `sensitive=False`, and otherwise the `Secret`'s `.value` with `sensitive=True`. `isinstance(text, str)` tells them apart, because a `Secret` is not a `str`.
- The binding's keyword-only `sensitive` selects the error's sensitive rendering, which `KeyboardActionError` offers: the position and the kind of failure (the sequence does not parse, a key cannot be typed, sending failed), with the syntax hint where one applies. It contains no character, key name or device message text. An error without a position is given by its kind: `NotReady` and `InputInProgress` keep their fixed texts (no keyboard device is ready; a keyboard input is already active), and any other device error reads "starting the input failed" when `KeyboardEngine::new` raised it and "sending failed" otherwise.
- The keyword line gives the length of a string as written, and says only `secret` for a `Secret`, as RF itself discloses nothing about a `Secret`.
- A `Secret` uses the same sequence syntax as a string: `\\` types a backslash; `\<` and `\>` type `<` and `>`; `\xHH` and `\uHHHH` type the character with that code, and are an error when not followed by 2 or 4 hex digits; a backslash before any other character is dropped (maintainer decision: this stays lenient). The keyword docs say so.
- A `Secret` must be the whole argument. Embedded in a longer argument, RF inserts the text `<secret>`, which the grammar reads as a key name. `Keyboard Type` is called once for the `Secret` and once for `<Return>`, or the whole sequence is built as a `Secret` with `VAR    ${sequence: Secret}    %{PASSWORD}<Return>`. The keyword docs say so.
- The minimum stays RF 7.0 (maintainer decision). A pytest in a subprocess replaces `robot.api.types` with a stub without `Secret`, as on RF < 7.4, to exercise the older path; Robot Framework's own `BuiltIn` needs the rest of that module.

**What RF itself shows.** RF records a keyword's arguments as written in the test data: a literal verbatim, a variable by its name. At `--loglevel TRACE` it also logs their values, and only a `Secret` is hidden there. A value also appears wherever the suite assigns or logs it. Repeating the argument therefore adds nothing, and only a `Secret` keeps the value out of a TRACE log. The concept tells users so.

*Alternatives rejected:*

- Keeping every entered value, its key data and field contents out of all records and messages, with masking and third-party ceilings. The maintainer rejected it as too strict for a test system. The sweep's other findings are kept in `review-findings.md`.
- Redacting a `Secret` in Python by string replacement. The message may name a single character, and replacing single characters would corrupt it.
- Passing `sensitive` into the runtime and the devices, so that their records could name keys of plain strings at debug. Trace is where per-key records belong anyway.

### 8. Each component checks its own configuration

Maintainer decision: the component that reads a setting checks it. Core gains, log-free:

- `ConfigMap::unknown_keys(&self, known: &[&str])`, which always treats `enabled` as known, a key reserved on every provider;
- `try_bool`, `try_i64`, `try_str` and `try_map(key) -> Result<Option<_>, ConfigTypeMismatch { key, expected, found }>`.

Every PlatynUI component checks its section at the start of its build, before anything can fail. It warns for each unknown key and each mismatch, naming `component` (`platform.x11`, `providers.java`) and `key`, and applies the default. A component that is not built is not checked, which is the "active" rule of runtime-session-config, and a warning is logged even when the build then fails. The known keys are constants that the reads themselves use, so the list cannot drift from the code.

- X11 reads `display`, now with `try_str`. Wayland, the Windows platform, the mock platform, the mock provider and macOS AX read nothing and check for unknown keys only.
- AT-SPI reads `bus_address` and `surface_popups`; UIA reads `honor_window_claims`.
- Java reads `enabled`, `agent` and `jab`, the latter two through `try_map`. Its backends already receive their own maps (`AgentBackend::from_config`, `JabProvider::from_config`) and check them there, reporting `agent.<key>` and `jab.<key>`.

The runtime reads `platform.backend` with `try_str` and warns on a mismatch. `select_platform` returns the chosen factory's id with the bundle, for the initialization record. Unclaimed component ids stay debug.

The Python binding reports what it cannot pass on: `parse_runtime_config` warns for a top-level key other than `platform` and `providers`, naming the key and the accepted buckets; for a bucket that is not a dict, naming the bucket and its Python type; and for a non-string key, a leaf or a list element of an unsupported type, naming the dotted path and the Python type. These would otherwise be dropped before any component sees them.

`enabled` is reserved on every provider. The open `runtime-provider-selection` change adds the bucket-level keys `include` and `exclude`, which the unclaimed-id records must skip, and records the active and suppressed providers; those belong into the runtime's single info record (fields `providers`, `suppressed`). See Open Questions for its warning on portable include entries.

**Profiles.** The extractors raise `TypeError` for a wrong type, naming the key and the expected type, and warn for an unknown key. The call that received the dict delivers the warning.

*Alternatives rejected:*

- Declaring keys on the factory traits (`config_keys()`) and flattening sections in the runtime. It changes both public factory traits, needs a second mechanism for types, and a test that compares each list with the reads; checking where the keys are read needs none of that.
- Raising for unknown profile keys like RF's `TypedDict`. It would break suites carrying a harmless extra key; a warning catches the typo.

### 9. Python logging goes through `logging` under `platynui.<module path>`

Python loggers take `platynui.` plus the module path in lower case (maintainer decision): `platynui.devices` becomes `platynui.core.adapter_devices`, `platynui.ui.application` already fits, and BareMetal logs through `platynui.baremetal`. Keyword modules use `logging` too. Under Robot Framework a `logging` record reaches the same place as `robot.api.logger`, and only a logger offers `isEnabledFor`, which decision 6 needs. Outside Robot Framework, `robot.api.logger` writes to a logger named `RobotFramework`, outside the `platynui` hierarchy that `dev-docs/python-bindings.md` promises. `robot.api.logger` stays for keyword output that needs Robot Framework features, such as the HTML screenshot embedding.

A deprecation that a Robot Framework user can hit, whether a keyword, a keyword argument or an import argument, is reported as a warning in the log, once per run and use site; a deprecated keyword's documentation starts with `*DEPRECATED ...*`, which Robot Framework reports itself. Robot Framework does not capture Python warnings, so `warnings.warn` would hide it from the people it concerns. `warnings.warn` is for the Python API (`platynui_native`, `PlatynUI.core`, `PlatynUI.ui`): `DeprecationWarning` for deprecations, and a `UserWarning` subclass for misuse.

### 10. Failed activation at DEBUG, everywhere

`_maybe_bring_to_front` logs the element (`describe()`) and the error at debug and continues (maintainer decision). It describes the element only when DEBUG is enabled (decision 6).

The UIA activation that did not reach the foreground window (`crates/provider-windows-uia/src/node.rs:596`) is also recorded at debug, not warn. It runs for automatic activation too, and a warning there would contradict that decision.

### 11. No lane report; runs are checked with the existing tools

Maintainer decision: no reporting tool. Robot Framework already prints every warning on the console during a run, `log.html` lists them under *Test Execution Errors*, and `robotcode results log --level WARN --execution-messages` lists them from `output.xml`, per test. A dedicated script would only have grouped and counted them, which does not justify the script, its tests, its wiring into recipes or a listener, and a spec requirement.

The rule that a healthy run has no PlatynUI warning stays in the spec. Task 6.2 checks it after each lane, and `dev-docs/logging.md` shows how to look. If noise goes unnoticed later, a separate change can add an automatic check.

### 12. The high-priority fixes, adjusted

Each finding is applied as verified by the review, with these adjustments:

- **Auto-activation:** WARN once becomes DEBUG (decision 10). The UIA foreground check becomes DEBUG (decision 10).
- **Pointer summary:** INFO becomes DEBUG (decision 6).
- **AT-SPI:**
  - The timeout helper no longer logs for timeouts its callers return.
  - The connection code's `error!` calls for returned failures (`connection.rs:32,36`) become debug.
  - Per-node reads use the provider-owned per-bus latch (decision 3).
- **Keyboard error:** the warn before releasing keys (`crates/runtime/src/keyboard.rs:43`) becomes debug, because the keyboard failure is returned. An `error!` is added only when the release itself fails. Both give the number of keys only: key codes are opaque to the engine, and a held key can be a character of a `Secret` (decision 7).
- **Assertion prefix:** `_assertable.py:121` passes `assertion_message or ''`, so a failure without a message no longer starts with `None`.
- **Double reports:** `crates/platform-windows/src/pointer.rs:98-99`, `screenshot.rs:89-90` and `crates/platform-linux-x11/src/screenshot.rs:41-42` become debug. These are the old rule document's own examples. On Windows the error is read before it is logged, so that no subscriber runs between `SendInput` and `GetLastError`, and the screenshot error carries the `windows::core::Error` in its details.
- **Missing platform:** the runtime's errors for a missing platform backend name no internal types: "runtime has no platform backend (none could serve this session, or the runtime is shut down)".
- **Test build:** in a `mock-provider` build without the mock backend, the test-build warning is the one report, once per process; the no-provider and no-backend records are debug for that construction. The extension decides this and passes it to the runtime as a construction option; `platynui-runtime` does not decide it from its own `mock-provider` feature, so the CLI's and the Inspector's mock builds keep both warnings.
- **Java Access Bridge:** a missing DLL is debug at startup. The JAB backend still reports the `SunAwt*` windows it can see as unserved, with the cause, and their pids, so that automatic attachment can reach those JVMs. Only the Java provider's router knows whether another backend served a window, so it warns, once per process, when no backend serves one, naming the DLL remedy instead of its `jabswitch` hint. An agent-served Swing window gives no warning.
- **Not found:** the target text stays `No element matched {query!r} within timeout of {timeout} seconds.`, which an existing suite matches (`tests/BareMetal/wait_keywords.robot:28`). A root set by `Set Root` that is not found raises its own subclass, which `Wait Until Exists` does not re-wrap, with `The root set by Set Root, {root_query!r}, was not found within timeout of {root_timeout} seconds; {query!r} was not evaluated.`, where the timeout is the one the root was resolved with.
- **Java agent:** only timeouts, transport, protocol and no-agent errors count toward degraded. An answer with an error proves the agent is alive.
- **Windows attach stub:** leaked, not freed, while the remote thread may still run (a crash fix). The log is debug, because the timeout is returned.
- **Constructors and shutdown:** `Runtime(...)`, `Runtime.new_with_mock()` and `Runtime.shutdown()` deliver queued native records before they return or raise, so construction-time warnings reach Python without waiting for the next call.

## Risks / Trade-offs

- **[`--log-level debug` and `PLATYNUI_LOG_LEVEL=debug` stop showing zbus and wgpu]** → Intended (decision 5) and documented. `RUST_LOG` remains the way into third-party crates.
- **[`PLATYNUI_LOG_LEVEL` no longer takes directives]** → Nothing in the repository uses them for the library. The rejection names `RUST_LOG`, and the migration plan lists it.
- **[Wrongly typed profile values now raise]** → Such suites silently ran with default timing. The error names the key and the type. Noted in the migration plan.
- **[A component forgets to call the check]** → Every PlatynUI factory has a unit test that builds it with a bogus key and asserts the warning. Third-party components are not checked.
- **[AT-SPI latch threading touches many call sites]** → The helper signature takes the latch, bus name and call, so a site cannot forget them. The task lists the call sites.
- **[The compositor lane hands `PLATYNUI_LOG_LEVEL` to the compositor]** → `scripts/startcompositor.sh` passes it as the compositor's `--log-level`, which accepts only its five lower-case names and opens every crate. `PLATYNUI_LOG_LEVEL=debug` makes the compositor log third-party debug output, and values the library accepts (`WARNING`, `off`, `critical`) stop the lane from starting. `dev-docs/logging.md` says to use `native_log_level` or `RUST_LOG` for the library in that lane, until the development tools adopt `platynui-log-filter`.
- **[Renamed Python logger]** → Affects only configurations naming `platynui.devices`. The concept says so.
- **[Existing records at the wrong level]** → The level requirements bind the records this change adds or changes, like the element description. The follow-ups in `review-findings.md` move the rest, for example the slow-call warning of `apps/inspector/src/model/tree_data.rs:399` and the warning for a returned failure at `crates/platform-linux-x11/src/window_manager.rs:190`.
- **[Existing native element fields do not use the description form yet]** → For example `crates/runtime/src/xpath.rs:396-404`. The requirement binds new and changed diagnostics. The follow-ups in `review-findings.md` bring the rest in line.
- **[Archive order]** → This change modifies `native-logging`, which exists as a main spec only once `native-log-bridge` is archived. Archive that change first.

## Migration Plan

- **Behavioral where stated:**
  - the level knob;
  - raising profile types;
  - the renamed Python logger;
  - new warnings where there was silence;
  - fewer warnings where there was flooding;
  - `PLATYNUI_LOG_LEVEL` takes a single level; directives go to `RUST_LOG`;
  - `error` and `off` apply to every module;
  - an unknown top-level configuration key and a wrongly typed setting are warned about;
  - keyboard errors that give the position and no longer repeat the input;
  - the not-found text for a `Set Root` root;
  - per-key records at trace instead of debug.

  No keyword, argument or return value changes. The keyboard keywords additionally accept a `Secret`.
- **Needs a native rebuild:** `UiNode.describe()`, the profile extractors, the filter builder, the constructor delivery and the `sensitive` keyboard argument live in the extension.
- **Sequence:**
  1. Archive `native-log-bridge`.
  2. Concept and rules.
  3. Core building blocks and the filter crate.
  4. Extension and binaries.
  5. The high-priority fixes per area.
  6. The Python library and its documentation.
  7. Verification.
- **Rollback:** each area's fixes are separate commits and revert on their own. The concept and the building blocks are additive.

## Open Questions

- The shared slow-call thresholds (200 ms in the rules, 1000 ms in AT-SPI). The threshold values are open; the level is not: debug per slow call, a warning once per episode for a subject that stays slow. No high-priority fix needs them, and the follow-ups set the constants.
- Whether the Windows lane shows further PlatynUI warnings. That is answered on a Windows machine with `robotcode results log --level WARN --execution-messages` (task 6.3).
- Whether a `Secret` should be typed as literal text, escaped by the keyword before it reaches the binding. This change treats it as a sequence like any string (decision 7), and the maintainer kept the lenient backslash rule, so a stray backslash in a `Secret` is dropped silently; the keyword docs say so. The escaping of the `set_text` proxies in `review-findings.md` raises the same question.
- `runtime-provider-selection` warns for an include entry naming a provider that is not registered on this platform. Under this concept such a portable entry is debug. Settle it in that change before either change is archived.
