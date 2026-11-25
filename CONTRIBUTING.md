# Contributing to PlatynUI

Thanks for helping build PlatynUI! This guide describes how to get set up, our coding standards, and the quality gates we expect before merging.

## 1) Prerequisites

- Rust: Stable toolchain via rustup; Rust 1.95 or newer (`rust-version` in `Cargo.toml`).
- [cargo-nextest](https://nexte.st): Test runner behind `just test`, `just test-crate`, and the other Rust test recipes. Install with `cargo install cargo-nextest --locked` or `cargo binstall cargo-nextest`.
- Windows: the MSVC build tools (Visual Studio or its Build Tools with the C++ workload). The Windows acceptance lane also needs their x86 libraries and the `i686-pc-windows-msvc` target (§8).
- Linux: the system packages the CI jobs install ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)); the acceptance job lists what the X11 and compositor lanes need on top.
  - `just bootstrap` builds PyGObject, the bindings of the GTK test apps, from source, because PyPI has no Linux wheels for it. That needs a C compiler, `pkg-config`, girepository-2.0 and cairo headers, on Debian/Ubuntu `sudo apt install libgirepository-2.0-dev libcairo2-dev pkg-config gcc`. `just bootstrap` checks them first and names what is missing. girepository-2.0 comes with GLib 2.80, so distributions with an older GLib, such as Ubuntu 22.04 or Debian 12, cannot set up the environment.
  - The GTK test apps also need GTK 3, GTK 4.18 or later, and their introspection data at run time, on Debian/Ubuntu `sudo apt install libgtk-3-0 libgtk-4-1 gir1.2-gtk-3.0 gir1.2-gtk-4.0`.
- Python: 3.12+ and uv >= 0.11.7. Do not use pip directly in this repo.
- Java, only for the Java agent, the Swing test app, and the Windows acceptance lane: any `java` 8 or newer on `PATH`. The Gradle wrappers provision the rest themselves, which needs network access on the first build. `just build-native` needs no JDK.
- [just](https://github.com/casey/just): Task runner for common dev workflows. Install with `cargo install just`, `brew install just`, `winget install Casey.Just`, `scoop install just`, or your system package manager (`pacman -S just`, `apt install just`, etc.).
- [git-cliff](https://git-cliff.org): Changelog generator, needed only for changelog and release work (`scripts/update-changelog.py`). Install with `cargo install git-cliff`, `cargo binstall git-cliff`, or `brew install git-cliff`. It must be on the `PATH` (the cargo installs put it into `~/.cargo/bin`): `cz bump` stops when the changelog step cannot find it.
- Tools: cargo, uv, and (recommended) GPG for signed commits.

Run commands from the repository root. `just` is the primary entry point for local development tasks; it wraps the expected `uv`, `cargo`, and `maturin` commands so everyone runs the same workflow.

Set up the environment:

```bash
just bootstrap
```

This creates `.venv`, or brings it up to date, with an exact `uv sync`: the dev tools (ruff, mypy, pytest, maturin, pre-commit, RobotCode with Robot Framework, PySide6 for the Qt test apps, and on Linux PyGObject for the GTK test apps) and the third-party dependencies of every workspace package, but none of the workspace packages themselves. Build what you need next: `just build-native` for the real desktop (the test recipes build their own `mock-provider` variant) and, for Java work, `just install-provider-java`. Because the sync is exact, a later `just bootstrap` removes those builds again — and every push runs one through the `pre-push` hook. [Run PlatynUI from a source checkout](#run-platynui-from-a-source-checkout) lists everything that undoes this setup.

## 2) Project layout (quick orientation)

- Rust workspace in `crates/*`, `apps/*`, and `packages/*` (core, xpath, runtime, providers/platforms, cli, inspector, and the crates behind the native, CLI, and Inspector wheels); the Python and Java test apps under `apps/` and `packages/provider-java` are excluded (`Cargo.toml`).
- Python packages in `packages/*` (native bindings, CLI, inspector, and `provider-java`, a pure-data wheel with no Rust in it) and RF library entry in `src/PlatynUI`.
- Java products in `java/*` — currently `java/agent`, the agent PlatynUI loads **into** a target JVM. Self-contained Gradle project; not a Cargo crate. (Java *fixtures* live under `apps/`, not here.)
- Developer, design, and planning docs live under `dev-docs/` ([index](dev-docs/README.md)); `docs/` is reserved for user-facing documentation; some crates keep component-local `docs/` directories. Which backend implements what on each platform is in the [platform support matrix](dev-docs/architecture.md#platform-support-matrix).
- Specifications live under `openspec/specs/` (the behavior users can rely on); proposed and in-progress changes under `openspec/changes/`.
- Generated artifacts such as `target/`, `.venv/`, `dist/`, `results/`, wheel files, build caches, and the agent JAR staged into `packages/provider-java` should not be committed.

## 3) Contribution scope and expectations

PlatynUI is still preview-stage software. Good contributions keep the moving parts understandable:

- Keep changes focused on one problem or one coherent feature.
- Prefer small, reviewable PRs over broad refactors.
- Preserve existing public behavior unless the PR explicitly changes it.
- Update tests and docs when behavior, commands, packaging, or platform support changes.
- Avoid drive-by cleanups in unrelated files; save them for separate PRs.
- Call out platform assumptions in the PR, especially for Windows, Linux X11, Linux Wayland, and macOS work.

If you are unsure whether a design belongs in the current architecture, open an issue or draft PR first. Early discussion is cheaper than a large rewrite late in review.

### Planning larger changes with OpenSpec

Larger changes — new or changed behavior, or a design decision — are planned as an OpenSpec change before they are implemented. A change lives in `openspec/changes/<name>/`:

- `proposal.md` — why the change is needed, what it changes, and what it touches;
- `specs/` — the requirements it adds or changes, each acceptance criterion as one Given/When/Then scenario;
- `design.md` — the decisions, backed by code references;
- `tasks.md` — the work, tests first, ending with the `just` recipes that verify it.

`openspec/config.yaml` holds the rules these artifacts follow, and `openspec/specs/` is the current specification every change builds on. The `openspec` CLI comes from npm (`npm install -g @fission-ai/openspec`):

```bash
openspec list               # changes in progress
openspec show <name>        # a change or a spec
openspec validate <name>    # check a change before committing it
openspec archive <name>     # when it is done: merge its specs into openspec/specs/, move it to openspec/changes/archive/
```

In Claude Code and GitHub Copilot, the `opsx` commands (`.claude/commands/opsx/`, `.github/prompts/opsx-*.prompt.md`) walk through the same steps. Commits that touch only OpenSpec artifacts use the scope `openspec`, for example `docs(openspec): propose …`.

## 4) Branching, commits, and PRs

- Use Conventional Commits: `type(scope): subject` (e.g., `feat(runtime): add window resize action`). The scope names a component already in use — `runtime`, `inspector`, `provider-atspi`, `java-agent`, `openspec`, … (`git log --format=%s` shows them) — never a directory or path; the `commit-msg` hook checks only the type.
- Keep subjects ≤ 72 chars. The body explains in a few lines why the change exists — what was wrong or missing, and why it matters; the diff already shows what changed. Link issues/PRs.
- PlatynUI is 0.x: no `feat!:`/`fix!:` subjects and no `BREAKING CHANGE:` footers. Name behavior changes that users will notice in the body and in the PR, so the release notes can list them.
- Sign commits when possible (`git config commit.gpgsign true`).
- Small, focused PRs with clear rationale and “how to verify” notes.

PR descriptions should include:

- What changed and why.
- User-visible behavior changes, if any.
- Platforms affected or tested.
- Commands run, preferably using `just` recipes.
- Known gaps, skipped checks, or follow-up work.

## 5) Dev workflow with `just`

Run `just` without arguments to list the recipes available on your OS. Recipes marked `[linux]`, `[unix]`, or `[windows]` in the `justfile` exist only there, though several `[windows]` recipes have a same-named `[unix]` twin. The `justfile` is the source of truth, and this section documents the contributor-facing workflow.

Use recipes first, and drop down to raw `cargo`, `uv`, or `maturin` commands only for targeted debugging or when a recipe does not exist yet. If a raw command becomes part of the normal workflow, add a `just` recipe and update the docs.

Common recipes:

| Goal | Recipe | Notes |
|---|---|---|
| List workflows | `just` | Shows recipes from the `justfile`. |
| Bootstrap dependencies | `just bootstrap` | Exact `uv sync` of the dev tools and third-party dependencies; installs no workspace package and removes those other recipes installed — the native module, the CLI and Inspector, and `platynui-provider-java` (see §1). |
| Format, lint, and type-check | `just check` | Runs `just fmt` (rewrites Rust files in place), `just clippy`, `just ruff`, and `just mypy`. |
| Format Rust code | `just fmt` | `cargo fmt --all`; rewrites files in place. |
| Check Rust formatting | `just fmt-check` | Fails on unformatted Rust code without touching files; the commit hook and CI run this one. |
| Rust lints | `just clippy` | Clippy over the whole workspace with warnings as errors, for the host OS only; CI runs it on Linux and Windows. |
| Python lints | `just ruff` | `ruff check` over the paths in `[tool.ruff] include`; lints only, does not format. |
| Python type check | `just mypy` | Strict mypy over `[tool.mypy] files`, plus one run per Python test app, because they all share the module name `main`. |
| Rust tests | `just test` | Runs the Rust workspace test suite via nextest. |
| One Rust crate | `just test-crate platynui-xpath` | Replace the package name as needed. |
| Python tests | `just test-python` | Builds the native package with `mock-provider`, which stays installed (§8), then runs pytest. |
| BareMetal RF (mock) tests | `just test-baremetal` | Robot suites under `tests/BareMetal` against the built-in mock tree (`mock` profile); builds `mock-provider`, no display needed. Extra arguments are appended to `robotcode --profile mock run`. |
| Rust and Python tests | `just test-all` | Runs `just test` and `just test-python`. |
| Acceptance tests | `just test-acceptance` | Real-provider suites under `tests/acceptance` (egui, Inspector, Qt Widgets, and Qt Quick; Windows adds Swing and the Win32 test window) against the non-mock build. Linux: compositor, X11, then X11 without a window manager — like any `just` dependency chain it stops at the first failing lane. See §8 for backends, headless, CI, and the Windows prerequisites. |
| Robot run summary | `just test-summary` | Prints a Markdown summary of the latest Robot Framework run from `results/output.xml`. It has no build step of its own, but its `uv run` syncs `.venv` first. Extra arguments go to `robotcode results summary`; CI appends it to the job summary. |
| Full local gate | `just pre-commit` | Runs bootstrap, checks, Rust tests, and pytest — not `just test-baremetal` or an acceptance lane; the `pre-push` hook runs it on every push. Leaves the `mock-provider` build installed and `platynui-provider-java` removed. |
| Cross-target gate | `just pre-commit-cross` | Linux-only; adds Windows and macOS ARM cargo check/clippy passes. |
| Install git hooks | `just hooks-install` | Installs `pre-commit`, `commit-msg`, and `pre-push` hooks. |
| Install push gate | `just hooks-install-push` | Alias for `just hooks-install`; the push gate is standard. |
| Enable Linux cross-target push gate | `just hooks-cross-enable` | Opts in to cross-target checks before every push on Linux. |
| Disable Linux cross-target push gate | `just hooks-cross-disable` | Turns the optional Linux cross-target push gate off again. |
| Native mock build | `just build-native-mock` | Needed before Python/RF work that uses `Runtime.new_with_mock()`. |
| CLI or Inspector build | `just build-cli`, `just build-inspector` | Builds local binary Python packages with maturin. |
| Clean local artifacts | `just clean` | Removes build/test artifacts while keeping `.venv` and tool caches. It also keeps the installed native module, the Gradle outputs under `java/agent/build` and `apps/test-app-swing/build`, and the staged agent JAR, so it does not reset a mock build or a stale JAR. |

Additional build and packaging recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Rust workspace build | `just build` | Builds all Rust crates and targets. |
| Native package build | `just build-native` | Builds the native module with the real platform providers and installs it into `.venv` (maturin with uv). Needed for real-desktop runs, and again after a test recipe or a push left the mock build. |
| Native package with feature | `just build-native mock-provider` | Passes optional Cargo features through to maturin. |
| Native wheel | `just build-native-wheel` | Builds a release wheel into `dist/`. |
| CLI wheel | `just build-cli-wheel` | Builds a release wheel for `platynui-cli`. |
| Inspector wheel | `just build-inspector-wheel` | Builds a release wheel for `platynui-inspector`. |
| Robot Framework wheel | `just build-platynui-wheel` | Builds the pure Python Robot Framework package wheel. |
| All local Python packages | `just build-all-python` | Builds native, CLI, and Inspector packages for local development. |
| All wheels (release build) | `just build-all-wheels` | Builds this host's wheels into `dist/` (pure-Python, native, CLI, Inspector, provider-Java) at the committed version — a packaging check (§10). Needs a `java` 8+ for the agent JAR. |
| Provider-Java wheel | `just build-provider-java-wheel` | Builds and stages the agent JAR (`just build-provider-java`), then builds the `platynui-provider-java` wheel; the JAR is mandatory, so this needs a `java` 8+. |
| Rust API docs | `just doc` | Builds Rust API documentation without dependencies. |

The debug-default build recipes — `build`, `build-native`, `build-cli`, `build-inspector`, and `build-native-mock` — honor a `release` variable: pass `release=true` to compile in release mode instead of debug, e.g. `just release=true build-all-python` (which inherits the flag through its dependencies). Because the acceptance lane also depends on `build-native`, `just release=true test-acceptance` runs those suites against an optimized native module. The `*-wheel` recipes ignore the flag and always build with the `dist` profile from the root `Cargo.toml`: the same speed optimizations as `release`, plus whole-program LTO, a single codegen unit and stripped symbols for smaller wheels, at the cost of a longer build; `build-native-wheel`, `build-cli-wheel`, and `build-inspector-wheel` pass extra arguments to `maturin build`.

Git hook recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Install standard hooks | `just hooks-install` | Installs `pre-commit`, `commit-msg`, and `pre-push` hooks from `.pre-commit-config.yaml`. |
| Install push gate | `just hooks-install-push` | Alias for `just hooks-install`; kept as an explicit push-gate command. |
| Run hooks manually | `just hooks-run` | Runs the `pre-commit` stage hooks against all files. |
| Run push hook manually | `just hooks-run-push` | Runs the `pre-push` gate without pushing, with the same side effects on `.venv` as a push (see below). |
| Run cross-target hook manually | `just hooks-run-cross` | Linux-only; runs the optional cross-target checks directly. |
| Enable cross-target push checks | `just hooks-cross-enable` | Linux opt-in; makes pre-push run Windows and macOS ARM checks too. |
| Disable cross-target push checks | `just hooks-cross-disable` | Removes the local opt-in flag. |
| Pre-push cross-target step | `just hooks-pre-push-cross` | Entry of the second `pre-push` hook (after `just pre-commit`), not for direct use: on Linux with the opt-in flag set it runs `just cross-target-checks`, otherwise it prints why it skipped. |
| Update hook revisions | `just hooks-update` | Updates remote hook revisions in `.pre-commit-config.yaml`. |
| Remove hooks | `just hooks-uninstall` | Removes installed hooks managed by `pre-commit`. |

Linux desktop integration recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Install desktop files | `just install-desktop` | Installs `.desktop` files and icons under `$XDG_DATA_HOME` or `~/.local/share`. |
| Remove desktop files | `just uninstall-desktop` | Removes the locally installed desktop files and icons. |
| Refresh icon cache | `just update-icon-cache` | Refreshes the GTK icon cache after install/uninstall. |

`just install-desktop` installs the entries for `org.platynui.compositor` and `org.platynui.inspector`. The egui test app's application ID is `org.platynui.test.egui` (its `--app-id` default); it has no desktop file.

Linux cross-target recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Check Windows crates | `just check-windows` | Cargo-checks Windows-relevant crates from Linux. |
| Clippy Windows crates | `just clippy-windows` | Runs clippy for Windows-relevant crates from Linux. |
| Check macOS ARM crates | `just check-macos-arm` | Cargo-checks macOS ARM-relevant crates from Linux. |
| Clippy macOS ARM crates | `just clippy-macos-arm` | Runs clippy for macOS ARM-relevant crates from Linux. |
| All four checks | `just cross-target-checks` | Runs the four recipes above; `just hooks-run-cross`, `just pre-commit-cross`, and the opted-in `pre-push` hook call it. |

The default cross targets can be overridden with `PLATYNUI_WINDOWS_TARGET` and `PLATYNUI_MACOS_ARM_TARGET`.

PID-namespace checks cover the deployment in which PlatynUI and the application run in different PID namespaces (see `dev-docs/platform-linux.md`). They are `#[ignore]`d, local only, and not part of `just test`. Each needs `unshare` with unprivileged user namespaces, and a missing prerequisite fails the run with a message naming it instead of skipping:

| Goal | Recipe | Notes |
|---|---|---|
| Compositor | `just test-compositor-pidns` | A Wayland client whose process the compositor cannot see. |
| AT-SPI process identity | `just test-atspi-pidns dbus-daemon` / `dbus-broker` | One run per bus implementation; dbus-broker also needs a user session. |
| X11 own-window decision | `just test-x11-pidns` | Needs `Xvfb`. |
| Wayland compositor identification | `just test-wayland-pidns` | The CLI in a sibling namespace of the compositor, through `scripts/wayland-sidecar-harness.sh`. Needs `dbus-run-session` and the AT-SPI binaries. |

Test app recipes (the applications the acceptance lanes drive; the Qt, QML, GTK, and Swing apps have READMEs with the details):

| Goal | Recipe | Notes |
|---|---|---|
| Run the QML test app | `just run-test-app-qml` | Starts `apps/test-app-qml` on the project venv (PySide6 is a dev dependency); extra arguments go to the app. |
| Run the GTK 3 test app | `just run-test-app-gtk3` | Linux only. Starts `apps/test-app-gtk3` on the project venv (PyGObject comes with `just bootstrap`); extra arguments go to the app, quoted values stay whole. |
| Run the GTK 4 test app | `just run-test-app-gtk4` | Linux only. Starts `apps/test-app-gtk4` on the project venv (PyGObject comes with `just bootstrap`); extra arguments go to the app, quoted values stay whole. |
| Build the Swing test app | `just build-test-app-swing` | Gradle wrapper; only a `java` 8+ on `PATH` is needed — the Gradle JVM, the JDK 21 compile toolchain, and the Java 8 launch runtime self-provision (network access on the first build). Writes `apps/test-app-swing/build/java-launchers.properties`, from which the lane and the live checks take the Java 8 path. |
| Run the Swing test app | `just run-test-app-swing` | Gradle `run` on the provisioned Java 8; on Windows the Java Access Bridge is enabled for this process only (no `jabswitch`, nothing persisted). Arguments mirror the Qt and egui apps; multi-word values need the Gradle wrapper directly (see the app README). |
| Swing runtime smoke | `just test-test-app-swing-runtimes` | Builds the app, then starts it on the provisioned Java 8 and JDK 21 with `--auto-close 3`; a failure on either runtime fails the recipe. |
| 32-bit Win32 test window | `just build-win32-test-window-x86` | Windows only; builds `apps/win32-test-window` for `i686-pc-windows-msvc`, the 32-bit process of the process-attribute suite. Prerequisites in §8. |

The Qt Widgets and egui apps have no run recipe: start the Qt app as [`apps/test-app-qt/README.md`](apps/test-app-qt/README.md) shows, and the egui app with `cargo run -p platynui-test-app-egui`.

Java agent recipes (see [`java/agent/README.md`](java/agent/README.md)):

| Goal | Recipe | Notes |
|---|---|---|
| Build the agent JAR | `just build-java-agent` | Gradle wrapper; only a `java` 8+ on `PATH` is needed, the rest self-provisions (network access on the first build). Writes `java/agent/build/libs/platynui-agent.jar`, which no installed package serves until it is staged (see below). |
| Agent unit tests | `just test-java-agent` | JUnit: element registry, toolkit-thread deadline, JSON layer. |
| Agent live checks | `just test-java-agent-live` | Every ignored test of `platynui-java-agent` against a real JVM: native attach, handshake discovery, and the delivery checks below. Builds the JAR, runs the JUnit tests, and builds the Swing test app first. |
| Delivery checks | `just test-provider-java-delivery` | Builds the real wheel, installs it into a throwaway venv, and resolves it through that environment's interpreter. Needs `uv`, restages the JAR into `packages/provider-java` on the way, and takes minutes. |
| Stage the JAR into its wheel | `just build-provider-java` | Builds the JAR and copies it into `packages/provider-java/src/platynui_provider_java/agent/`, where an editable install serves it at once. |
| **Enable the agent locally** | `just install-provider-java` | Stages the JAR *and* installs `packages/provider-java` into `.venv` (editable). **Needed to see the agent backend at work — and again after every `just bootstrap` or push, which remove it.** |

`just build-native` deliberately does **not** build the agent and stays JDK-free: a missing JAR is a runtime diagnostic ("install `robotframework-platynui[java]`", logged at debug level), never a build failure. Only the release/wheel recipes and the lanes that exercise the agent treat it as a hard prerequisite.

**Why the extra install step, and the trap it avoids.** Installing `platynui-provider-java` *is* the consent for in-JVM instrumentation, so nothing installs it implicitly — not `build-native`, not `build-inspector`. Without it the Java provider behaves correctly and confusingly: the agent backend is built and asks for a JAR, finds none, and the Access Bridge serves the window. So a Swing application inspected from a source build shows `@Technology = "JAB"` and looks exactly as it did before the agent existed. **A rebuilt JAR is not a delivered JAR.** `just build-java-agent` writes `java/agent/build/libs`, but the *installed* package keeps serving the copy staged under `packages/provider-java` until `just build-provider-java` restages it — and the provider↔agent version handshake cannot catch the difference, because both sides still report the same dev version. So an agent change can appear to have no effect. `just install-provider-java` does both steps; prefer it while working on the agent.

PlatynUI finds the installed package through the environment it runs in; [Run PlatynUI from a source checkout](#run-platynui-from-a-source-checkout) says how each entry point finds it and what removes it again. The agent's version in `java/agent/gradle.properties` is kept in lockstep by `scripts/update-git-versions.py` — provider and agent must match exactly, because an agent cannot be unloaded from a JVM.

The manual Java Web Start reproduction harness lives in [`scripts/webstart-repro/`](scripts/webstart-repro/README.md); it needs OpenWebStart and is never run by a lane or CI.

### Run PlatynUI from a source checkout

To run your own suite (here `report.robot`) against the real desktop with the code of this checkout:

```bash
just bootstrap
just build-native                     # native module with the real desktop providers
just install-provider-java            # optional: Java agent support (Windows)
uv run --no-sync robot report.robot   # --no-sync: do not let uv replace that build
```

Run other Python commands against that build the same way, for example `uv run --no-sync python -m robot.libdoc PlatynUI.BareMetal BareMetal.html` for the keyword documentation. `just build-inspector` and `just build-cli` install the Inspector and the CLI into `.venv`; start them from there, not from `target/`, so they find the agent package too.

**What undoes this setup.** `just bootstrap` is an exact sync: it removes what other recipes installed into `.venv` — the native module of `just build-native`, the CLI and Inspector of `just build-cli` and `just build-inspector`, and the agent package of `just install-provider-java`. The test recipes (`just test-python`, `just test-baremetal`, `just test-all`, `just pre-commit`) replace the native module with the `mock-provider` build, which links no real platform or providers and cannot drive the real desktop; a run on it warns that `platynui_native` is a test build. The `pre-push` hook runs `just pre-commit`, so every push does both. Run `just build-native` again afterwards, and `just install-provider-java` when you work with Java applications.

**Java applications (Windows).** A Robot Framework or pytest run finds the agent package only through `VIRTUAL_ENV`: the `python.exe` and the console scripts in `.venv/Scripts` are launchers that start the base interpreter outside `.venv`. `uv run` sets `VIRTUAL_ENV`, and so does an activated `.venv`. Starting `.venv/Scripts/robot.exe` directly, or from an IDE runner that does not export `VIRTUAL_ENV`, runs without the agent — the Access Bridge serves the window — and the miss is logged at debug level only. The Inspector and the CLI look in the environment they are installed in first and fall back to `VIRTUAL_ENV`, so a `cargo build` binary under `target/` finds the package only with `VIRTUAL_ENV` set. `PLATYNUI_JAVA_AGENT_JAR` overrides discovery with an explicit JAR path. The `VIRTUAL_ENV` dependence is a gap, not the design: `crates/java-agent/src/discovery.rs` describes an in-process lookup for runs inside Python, but nothing calls `set_in_process_resolver` yet.

### Git hooks with `pre-commit`

This repository uses [pre-commit](https://pre-commit.com/) as the Git hook runner. It is installed through the `uv` development environment; no global installation is required after `just bootstrap`.

Install the hooks with:

```bash
just hooks-install
```

The commit-time hook set is intentionally quick:

- file hygiene checks from `pre-commit-hooks` (`check-yaml`, `check-toml`, trailing whitespace, final newline, large files)
- `just fmt-check` for Rust formatting when Rust files changed
- `just ruff` for Python linting when Python files changed
- `just mypy` for Python type checks when Python files changed
- `conventional-pre-commit` in the `commit-msg` hook for Conventional Commit messages

Some file hygiene hooks can update files automatically. If that happens, review the changes, stage them, and commit again.

The full project gate is heavier, so it runs at Git `pre-push` instead of Git `pre-commit`. That hook runs `just pre-commit`, which includes bootstrap, checks, Rust tests, and Python tests (pytest; the RF mock suites run through `just test-baremetal` and in CI). You can run the same gate manually with `just pre-commit` or `just hooks-run-push`. Either way it leaves `platynui-provider-java` uninstalled and the `mock-provider` native build in `.venv`, so run `just build-native` and, for Java work, `just install-provider-java` again before working against the real desktop.

On Linux, contributors with the cross-target toolchain installed can opt in to an additional pre-push gate:

```bash
just hooks-cross-enable
```

That writes a local Git config flag and makes every push run the Windows and macOS ARM cross-target checks after the normal pre-push gate. The opt-in is local to your checkout and is not committed. Disable it again with `just hooks-cross-disable`, or run the cross-target checks manually with `just hooks-run-cross`.

To run these recipes on Linux, install the Rust targets and host tools first:

```bash
rustup target add x86_64-pc-windows-gnu
rustup target add aarch64-apple-darwin

# Debian/Ubuntu
sudo apt install gcc-mingw-w64-x86-64 llvm

# Arch Linux
sudo pacman -S mingw-w64-gcc llvm

# Fedora
sudo dnf install mingw64-gcc llvm
```

Windows cross checks require `x86_64-w64-mingw32-gcc` and `llvm-rc` on `PATH`. macOS ARM checks currently type-check/clippy the relevant crates only; they require the Rust target but not a full Apple SDK. The `just` recipes validate these prerequisites and print the missing command or package when something is not installed.

These Linux cross-target recipes are early compatibility checks, not release builds. Real platform binaries, wheels, installers, and release candidates must still be built and verified on the appropriate target platform or a dedicated native builder: Windows on Windows, macOS on macOS, and Linux on Linux.

Before pushing non-trivial changes, run:

```bash
just pre-commit
```

For quick iteration, run the smallest recipe that covers the touched area. Examples:

```bash
just test-crate platynui-xpath
just ruff
just test-python
```

Targets that change public behavior should include/update tests.

Recommended verification by change type:

| Change type | Recommended local checks |
|---|---|
| Documentation only | Check links and examples touched by the change; run `just` if documented commands changed. |
| Rust library/runtime change | `just check` and `just test-crate <package>`; use `just test` for shared behavior. |
| XPath parser/evaluator change | `just test-crate platynui-xpath` plus targeted tests under `crates/xpath/tests/it/`. |
| Python or Robot Framework change | `just test-python` and `just test-baremetal`; also run `just ruff` and `just mypy` for Python edits. |
| Native Python binding change | `just build-native-mock` and `just test-python`; add Rust tests if the Rust API changed. |
| Java agent or Java provider change | `just test-java-agent`, `just test-java-agent-live`, and `just test-crate platynui-provider-java`; on Windows `just install-provider-java`, then `just test-acceptance-windows` for the Swing suites. When the agent's loading or threading changes, also run the manual Web Start harness in `scripts/webstart-repro/`. |
| CLI or Inspector packaging change | `just build-cli` or `just build-inspector`; run relevant CLI/Inspector checks manually if behavior changed. |
| Platform/provider change | `just test` plus the acceptance lane of each affected OS and manual verification where the lane does not reach. Use `just pre-commit-cross` on Linux for cross-target checks. |
| Dependency change | Run the relevant build/test recipe and commit the changed lockfile (`Cargo.lock` or `uv.lock`). |

## 6) Coding standards

Rust:
- Edition 2024; follow existing naming (snake_case functions, PascalCase types).
- Prefer typed errors (thiserror) in library crates; avoid panics in normal flows.
- Error handling conventions are documented in [dev-docs/error-handling.md](dev-docs/error-handling.md).
- Keep JSON/serde usage consistent; do not add alternate JSON libs.
- Use `rstest` for fixtures/parametrization; keep tests small and deterministic.
- Keep public APIs documented through clear names and focused docs rather than broad comments.
- Re-export public surface from the relevant crate `lib.rs` when it is meant for external use.
- Use `tracing` for diagnostics; stdout is reserved for command output in binaries. Levels, log-or-return, and what a warning has to say are defined in [dev-docs/logging.md](dev-docs/logging.md); reviews apply its checklist, [`.github/instructions/logging.instructions.md`](.github/instructions/logging.instructions.md).
- Unsafe code is denied by default. If unavoidable for FFI or shared memory, keep it narrow and document the safety invariant.

Python:
- 3.12+; keep dependencies minimal. Lint with ruff; type-check with mypy in strict mode (`[tool.mypy]` in `pyproject.toml`), which `just check`, the commit hook, and CI enforce.
- Robot Framework keywords: Title Case (e.g., `Open Application`). Avoid `print`; return values instead.
- Use `uv` for environment and package workflows. Do not use `pip install` to mutate the repo environment.
- Keep the high-level Robot Framework API stable where possible; prefer additive changes during preview unless a breaking change is intentional.

CLI/Inspector (apps):
- Cross‑platform providers are linked via the `platynui_link_providers!` macro and Cargo target cfgs; follow the existing pattern.
- Keep stdout machine-readable when a command promises structured output; send logs and diagnostics to stderr.
- For UI or terminal output changes, include before/after notes or screenshots in the PR when useful.

## 7) Dependencies

- Rust: add to the crate’s `Cargo.toml`; build to update `Cargo.lock`.
- Python: edit `pyproject.toml`, then run `just bootstrap` rather than a plain `uv sync`: the recipe syncs the dependencies, groups, and extras of every workspace package (maturin, for one, comes from the `packages/native` dev group). Rebuild what it removed afterwards (§1). Commit both the `pyproject.toml` and updated `uv.lock`.
- Prefer small, widely‑used, stable libraries. Justify heavyweight deps in the PR.
- Before adding a Rust dependency, check whether the standard library already provides the needed functionality for the workspace's Rust version.
- Keep `tracing` as a per-crate dependency rather than a workspace dependency because of maturin compatibility constraints.
- Avoid adding dependencies only for tests if a small local fixture is clearer and cheaper.

## 8) Testing guidance

The test layering and the mock vs. real-provider lanes — what goes where, and why — are described in [dev-docs/testing-strategy.md](dev-docs/testing-strategy.md), together with the conventions for writing tests (§7 there). This section is the operational complement: what to run, and how the build duality affects your local loop.

Tests never drive applications that ship with the operating system — Notepad, Calculator, the taskbar: they change with OS updates and break tests for reasons that have nothing to do with PlatynUI. Use the test applications under `apps/`, or, in a Rust test, a window the test opens in a child process of its own.

Rust:
- Unit tests live alongside code; integration tests under `tests/` per crate.
- Use the mock provider/platform for deterministic tests. For manual runs, enable with `--features mock-provider` (some crates enable it via dev‑deps automatically).
- Prefer targeted tests close to the changed behavior, then broaden to workspace tests for shared contracts.
- Use `cargo nextest` through `just` recipes for normal local runs.

Python / RF mock lane:
- Python tests live under `tests/PlatynUI` and `packages/native/tests`; the deterministic RF mock suites live under `tests/BareMetal`. Run them with `just test-python` (pytest) and `just test-baremetal` (RF mock) — both build the `mock-provider` native package first.
- Tests that call `Runtime.new_with_mock()` (or import the library with `use_mock`) need that `mock-provider` build, or they error.
- Both recipes leave that build installed, as do `just test-all` and `just pre-commit` (and so every push); run `just build-native` before real-desktop work — the acceptance recipes do it themselves.

End‑to‑end / acceptance:
- The real lane needs the **non-mock** native build (a `mock-provider` build links no real platform or providers and cannot drive the real desktop); the `just test-acceptance*` recipes handle the build and, on Linux, an isolated session. They are **not** part of `just pre-commit` — run them separately:

  | Command | Scope |
  |---|---|
  | `just test-acceptance` | This OS — Linux runs the compositor, then X11, then X11 without a window manager, each only after the one before passed; Windows runs on the real desktop. |
  | `just test-acceptance-compositor` | Linux — under the PlatynUI Wayland compositor. |
  | `just test-acceptance-x11` | Linux — under an isolated X11/Xephyr session. |
  | `just test-acceptance-x11-bare` | Linux — under the same X11 session with no window manager running; leaves out the suites tagged `window-manager:required` and runs those tagged `window-manager:none`. |
  | `just test-acceptance-windows` | Windows — on the native desktop (UIA and Java providers), no isolated session. Before the Robot suites it builds the egui app, the Inspector, the Swing test app, the 32-bit Win32 window, and the agent JAR, then runs the agent's JUnit tests and the ignored live checks of `platynui-provider-java`, `platynui-java-agent`, and `platynui-process`. |

  Each recipe runs its lane profile — `real-wayland`, `real-x11`, `real-x11-bare` or `real-windows` — which excludes suites tagged for other platforms (`platform:*`, see `robot.toml`). On Linux the profile's `wrapper` also brings the isolated session up, and extra arguments are appended to `robotcode --profile real-wayland|real-x11 run`, e.g. `just test-acceptance-compositor --suite '*.Egui.WindowActivation'`. Robot Framework matches suite names normalized (spaces dropped), and `just` does not keep shell quoting when it forwards the arguments, so write names without spaces. On Windows extra arguments replace the default `--profile real-windows run` entirely.
- **Windows: the lane needs the 32-bit Rust target.** `just test-acceptance-windows` builds `apps/win32-test-window` for 32-bit x86, the 32-bit process of the process-attribute suite (`just build-win32-test-window-x86`). Install the target once with `rustup target add i686-pc-windows-msvc`. The MSVC x86 libraries come with the "x64/x86 build tools" of Visual Studio's C++ workload. Linux needs neither: the window never runs there, and the Linux cross checks compile it for Windows x64.
- **Windows: the lane also needs Java and the installed agent package.** It builds the Swing test app and the agent JAR with their Gradle wrappers, which need a `java` 8 or newer on `PATH` (and network access on the first build). Its Rust live checks get the freshly built JAR through `PLATYNUI_JAVA_AGENT_JAR`, and its delivery checks copy that JAR into `packages/provider-java` on the way, so an editable install serves it to the Robot suites as well. The recipe does not install `platynui-provider-java` itself: run `just install-provider-java` before the lane, and again after every `just bootstrap` or push. Without the package the agent suites (`tests/acceptance/swing/agent_*.robot` and `tests/acceptance/swing/process_attributes.robot`) fail waiting for `@Technology = "JavaAgent"`.
- **Headless / CI.** `headless=true` runs the Linux backends with no visible window (default under `CI`); it needs a GPU render node or Mesa software GL so egui can draw.
- **Linux: use a Linux host, not WSL.** The X11 lane's session does not work under WSL — Xvfb cannot bind its socket under WSLg, and Xephyr refuses the runtime's connection — so results from there say nothing about the lanes. On Windows, `cargo check` or `cargo clippy` with `--target x86_64-unknown-linux-gnu` is fine for compiling Linux code.
- **Windows: the session must stay connected and unlocked for the whole run.** This is specific to Windows because it is the one lane without an isolated session (see the table above) — it drives the real desktop and synthesizes real pointer and keyboard input for several minutes. A lock screen, a screensaver with "require sign-in", or a disconnected RDP session takes that desktop away mid-run.

  The failure looks like a permissions bug and is not one:

  ```
  PointerError: pointer action failed: platform capability unavailable:
    SetCursorPos: failed: Error { code: HRESULT(0x80070005), message: "Zugriff verweigert" }
  KeyboardError: keyboard provider is not ready
  ProviderError: platform capability unavailable: BitBlt: failed
  ```

  `0x80070005` is `E_ACCESSDENIED`, and here it means *nobody is at this desktop* rather than *you lack a right*. Two shapes, both observed: the whole lane collapses when the session was already locked at the start, or — if an idle timer fires mid-run — the suites that ran before it stay green and everything pointer/keyboard-dependent after it fails, which looks deceptively like a defect localised to one suite. Check `quser` for the session state before reading such a result as a regression, and disable the idle lock (or stay at the machine) for the duration.
- UI automation is platform-sensitive: include OS/session details (compositor vs X11, headless or not) when reporting failures or adding manual verification notes.

## 9) Adding or changing public APIs

- Rust public APIs: update crate modules and re‑export in `lib.rs` if part of external surface. Keep breaking changes minimal and documented in the PR.
- XPath engine changes: add targeted tests under `crates/xpath/tests/it/` following existing naming (e.g., `evaluator_*.rs`, `parser_*.rs`), and declare each new file in `crates/xpath/tests/it/main.rs`. The directory is one test binary, so an undeclared file is not built.
- Python RF library: the keyword surface in use is `PlatynUI.BareMetal` (`src/PlatynUI/BareMetal/`); the high-level `PlatynUI` library (`src/PlatynUI/__init__.py`) is still a placeholder in migration (see [dev-docs/python-migration-status.md](dev-docs/python-migration-status.md)). Keep keyword names stable, and document keywords in their docstrings, which libdoc publishes.
- Rust/Python boundary changes belong in `packages/native`; keep binding code out of core logic crates.
- CLI behavior changes should update help text, examples, and tests where practical.
- Platform-provider changes should state which backends are implemented, stubbed, or intentionally unsupported, and update the [platform support matrix](dev-docs/architecture.md#platform-support-matrix) together with the README's plain-language table.

## 10) Packaging and release (preview)

PlatynUI is built as five Python packages and published to PyPI by the `publish` workflow (`.github/workflows/publish.yml`): every night as a dev version, and on a version tag as a release. End-user install commands live in [README.md](README.md) and in the package READMEs (each package's PyPI page); this guide links there instead of repeating them.

| Package | Built from | Wheels |
|---|---|---|
| `robotframework-PlatynUI` | `src/PlatynUI` (root `pyproject.toml`) | One pure-Python wheel. |
| `platynui-native` | `packages/native` (PyO3, `abi3-py312`) | One per platform. |
| `platynui-cli` | `packages/cli` (maturin `bindings = "bin"`) | One per platform. |
| `platynui-inspector` | `packages/inspector` (maturin `bindings = "bin"`) | One per platform. |
| `platynui-provider-java` | `packages/provider-java` (data only: the JAR built from `java/agent`) | One `py3-none-any` wheel. |

- **One version for all.** `robotframework-PlatynUI` pins `platynui-native` and the packages behind its extras (`[cli]`, `[inspector]`, `[java]`, `[all]`) to exactly its own version, and the provider refuses an agent package or a running agent of any other version. The five therefore only work together at one version.
- **Versions come from git.** `cz bump` writes each release version into the manifests (its `pre_bump_hooks` run `scripts/update-git-versions.py`), so the committed version is the one the last bump wrote. Between bumps only CI applies git versions: every CI wheel job first runs `scripts/update-git-versions.py`, which writes the version that `scripts/tools.py` derives from `git describe --tags --long --first-parent --match 'v[0-9]*'` into every manifest, every exact pin, and `java/agent/gradle.properties` — a tagged commit gets its tag's version, each later commit `<next version>-dev.<commits since the tag>`. A new tag matching `v[0-9]*`, lightweight or annotated, therefore changes the version of every later build.
- **One CI run builds the whole set** — seven artifacts, 17 wheels: `wheels-pure-python`, `wheels-provider-java`, `wheels-linux-x86_64`, `wheels-linux-aarch64`, `wheels-macos-latest`, `wheels-windows-latest`, and `wheels-windows-11-arm`. CI itself uploads nothing; `publish.yml` publishes the wheels of one CI run, never a mix. Artifacts built from different commits carry different versions and do not satisfy each other's exact pins. The workflow name `CI`, its `v*` tag trigger and these artifact names are what `publish.yml` and `scripts/release_wheels.py` rely on, so change them only together.
- **Local wheels are a packaging check.** `just build-all-wheels` and the `just build-*-wheel` recipes build only this host's wheels, and because no `just` recipe runs `update-git-versions.py`, they carry the committed version, not the one CI derives for the same commit. During development, prefer the source build ([Run PlatynUI from a source checkout](#run-platynui-from-a-source-checkout)).
- Release and changelog automation (`cz bump` with the hooks in `[tool.commitizen]`, git-cliff through `scripts/update-changelog.py`, and `publish.yml`) is maintainer-owned unless explicitly coordinated in an issue or PR.

### Nightly dev builds

Every night at 02:23 Europe/Berlin, the `publish` workflow takes the newest CI run for a push to `main`. You can also start it by hand from the Actions tab.
- **Waiting:** while that run is still going, the workflow waits for it, at most two hours. A newer run that a push starts meanwhile takes its place.
- **Uploading:** if the run succeeded, its 17 wheels go to PyPI as a dev version. That version is the one `scripts/tools.py` derived for the commit, in its PEP 440 form: `0.14.0-dev.12` becomes `0.14.0.dev12`.
- **Order:** `platynui-native`, `platynui-cli`, `platynui-inspector` and `platynui-provider-java` go first, and `robotframework-PlatynUI` goes last. PyPI therefore never offers a root version whose exact pins cannot be met.

There is no dev build that night if any of these holds. Such a night still ends green, because CI already shows a red `main`.
- The newest run failed, was cancelled, or is not done within the two hours. An older green run is never taken, because during feature work it is often an unfinished intermediate state.
- PyPI already has that version.
- The run built a version without a `dev` label, such as the release commit's `1.0.0`. Its tag publishes that version.

Installers take a dev version only when asked for pre-releases, or when it is pinned:

```sh
pip install "robotframework-platynui[all]==0.14.0.dev12"
```

### Releasing a version

A version tag is a release, for example `v1.0.0` or the pre-release `v1.0.0-rc.1`. To release 1.0.0:

1. Check that `git-cliff` is on the `PATH` (`git-cliff --version`). `scripts/update-changelog.py` stops the bump without it. In that case the earlier hook has already rewritten the version places, so run `git restore .` and bump again.
2. For the step to 1.0.0 only: set `major_version_zero = false` in `[tool.commitizen]`. The bump's last hook runs `git add .` and takes this edit into the release commit. Left at `true`, commitizen keeps treating breaking changes as minor bumps.
3. Run `cz bump 1.0.0`. The bump:
   - writes the version into every manifest, every exact pin and `java/agent/gradle.properties`;
   - writes the `## [1.0.0]` section of `CHANGELOG.md`;
   - refreshes the lock files;
   - creates the signed release commit and the signed tag `v1.0.0`.
4. Push the branch and the tag together, for example `git push origin main v1.0.0`.

When the tag's CI run succeeds, the `release` job of `publish` takes over:
- It creates the GitHub release `v1.0.0` with that run's 17 wheels, and with the version's changelog section as notes. Above GitHub's limit of 125,000 characters, the notes keep only the subject lines of features, bug fixes and performance improvements, and link to `CHANGELOG.md`.
- It then uploads the same wheels to PyPI.

A pre-release tag goes the same way:
- Its GitHub release is marked as a pre-release.
- Its notes are the `[Unreleased]` section of its commit. The bump writes the changelog before the tag exists, so a pre-release has no section of its own.
- PyPI receives the PEP 440 form, `1.0.0rc1`.

A failed tag build publishes nothing.

### Dev tags and version lines

Dev versions come only from the nightly upload, so **do not tag `-dev.N` versions**. The release job refuses a tag such as `v0.14.0-dev.3`. After it, the nightly numbers would start again at `.dev1`, and those file names already exist on PyPI, or did once: PyPI never accepts a file name again, even after it was deleted.

The one exception is a `-dev.0` tag, which **starts a version line**. Every build after a tag derives its version from that tag. To leave the current line behind (0.13 from `v0.13.0-dev.2`) and start at 0.14, run `cz bump 0.14.0-dev.0` and push `main` and the tag. Then:
- the release job publishes nothing for that tag, and ends green;
- the nightly upload publishes `0.14.0.dev0` for the tagged commit, and `0.14.0.dev1`, `0.14.0.dev2` and so on after it.

These rank above every 0.13 version.

### One-time setup

Publishing uses PyPI's Trusted Publishing: the workflow gets a short-lived upload token through its GitHub identity, and the repository stores no PyPI credential. This needs two settings, both maintainer setup outside the repository:

- **GitHub environment `pypi`** (Settings → Environments):
  - deployment branches limited to `main`, so a workflow on any other ref cannot get a PyPI token. Both jobs of `publish` run on `main`, because GitHub runs `schedule` and `workflow_run` on the default branch;
  - no required reviewers, because the nightly job runs unattended;
  - no secrets.
- **A Trusted Publisher on PyPI** for each of the five projects (the project's Publishing settings, "GitHub"). It names:
  - the repository owner;
  - the repository name;
  - the workflow `publish.yml`;
  - the environment `pypi`.

  For a project that does not exist on PyPI yet, add a "pending publisher" under your account's Publishing settings.

Both settings name the repository. Set them up again when the repository moves to another owner or gets another name. Until then every upload fails with `invalid-publisher`, and PyPI's error lists the claims of the run to compare.

### Re-running a failed release

Fix the cause, then re-run the `release` job of the `publish` run:
- the upload skips every wheel PyPI already has, so only the missing ones are sent;
- the GitHub release is updated instead of created again.

A re-run uses the workflow file and the scripts of the original run. If the fix is in them, upload the missing wheels by hand from the GitHub release with `uv publish` and a PyPI API token of your own, and correct the workflow before the next tag.

### Cleaning up PyPI

PyPI allows 10 GB per project by default, and every nightly dev build adds about 58 MB to `platynui-inspector`. Delete old dev versions by hand on PyPI to stay below the limit:

- **Older dev versions** are safe to delete. Versions only rise, so no build ever needs one of their file names again.
- **Never delete the newest dev version** while no newer commit is on `main`. The next nightly upload would find it missing, and PyPI would refuse the upload with "Filename has been previously used". The nightly would then fail every night until the next commit.

## 11) Documentation

- Keep README files accurate and concise. Link to package READMEs for CLI/Inspector details.
- The root `README.md` is for users, and it is also the PyPI page of `robotframework-PlatynUI`. It covers what PlatynUI is, installing and using the published packages, Java support, diagnostics, and platform support. It carries no instructions for building, bootstrapping, testing, or running PlatynUI from a source checkout; those live in this guide, and the README links here at most. It stays non-technical: no crate names, operating-system APIs or protocols, and no links to `dev-docs/`.
- Architecture, design, and planning docs live under `dev-docs/` (alongside component-local `docs/` directories); `docs/` is where user-facing documentation will live. Update relevant docs with any non-trivial design change and add a brief English summary when possible.
- Public README files should orient users; keep deep implementation notes in `dev-docs/` or crate-specific docs, and do not link to `dev-docs/` from them. Each package's README, the root `README.md` included, is also its PyPI page, where relative links do not resolve: link to files in the repository with absolute URLs under `https://github.com/imbus/robotframework-PlatynUI/blob/main/`.
- When documenting commands, prefer `just` recipes for contributor workflows and package commands for end-user workflows.
- Keep docs in English unless updating an existing German planning document.

## 12) Security & privacy

- Do not commit secrets or personal data. Use environment variables or secure stores.
- Review dependencies for vulnerabilities; note relevant CVEs or fixes in PR descriptions when upgrading.
- Be careful with screenshots, UI tree dumps, logs, and Robot output; they can contain window titles, paths, hostnames, or user data.
- A `trace`-level log can name every key PlatynUI presses, secrets included; keep such logs out of public issues and PRs.

## 13) Troubleshooting contribution setup

- If Python tools are missing, run `just bootstrap` again, then rebuild what it removed: `just build-native` for the real desktop, `just build-cli` and `just build-inspector` if you start those from `.venv`, `just install-provider-java` for the agent.
- If mock Python tests fail because mock providers are unavailable, run `just build-native-mock` before retrying.
- If a run against the real desktop finds only the desktop node and warns that `platynui_native` is a test build, the native module is the `mock-provider` build that `just test-python`, `just test-baremetal`, or `just pre-commit` (and so every push) left behind: run `just build-native`.
- If a Swing application shows `@Technology = "JAB"` where you expect the agent, one of three things happened, each logged at debug level only: `platynui-provider-java` is not installed (`just bootstrap` and every push remove it; run `just install-provider-java`); the process cannot find it (Robot Framework and pytest need `VIRTUAL_ENV`, so start them with `uv run` or from an activated `.venv`; the Inspector and CLI need to run from `.venv` or with `VIRTUAL_ENV` set); or the attach failed, for example because the target runs elevated, under another user, or with another architecture (see [dev-docs/java-toolkits.md](dev-docs/java-toolkits.md#what-has-to-be-true-for-a-target-to-be-served)). `PLATYNUI_JAVA_AGENT_JAR` bypasses discovery. An agent that was injected but never answers, or one from another PlatynUI version, is reported as a warning.
- If building or syncing the native module fails on Windows with `os error 32` ("the process cannot access the file"), another process has `platynui_native`'s `_native.pyd` loaded — typically the RobotCode language server in VS Code, or a Python, pytest, or Robot run still going. Stop it and retry.
- If a Gradle recipe fails on Linux or macOS with exit code 126, a `gradlew` was committed without its executable bit — typical for a Gradle project set up on Windows, where the bit does not exist. Set it with `git update-index --chmod=+x <project>/gradlew`; `git ls-files -s <project>/gradlew` then shows mode `100755`.
- If Linux accessibility trees are empty, make sure AT-SPI is enabled and running for X11/XWayland sessions.
- If Linux cross-target recipes fail, install the missing Rust target or system compiler named in the recipe error.
- If a `just` recipe is too broad for investigation, run the equivalent raw command temporarily and capture the result in the PR notes.

---

Questions? Open an issue or start a discussion. Thank you for contributing to PlatynUI!
