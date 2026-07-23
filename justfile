# PlatynUI development task runner
# See CONTRIBUTING.md for contributor workflow details.

set shell := ["bash", "-euo", "pipefail", "-c"]
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# XDG data directory for local installs (only meaningful on Linux)
xdg_data_home := if os() == "linux" { env("XDG_DATA_HOME", env("HOME") / ".local" / "share") } else { "" }
python_executable := if os() == "windows" { justfile_directory() / ".venv" / "Scripts" / "python.exe" } else { justfile_directory() / ".venv" / "bin" / "python" }
export PYO3_PYTHON := env("PYO3_PYTHON", python_executable)
windows_rust_target := env("PLATYNUI_WINDOWS_TARGET", "x86_64-pc-windows-gnu")
windows_rust_packages := "--package platynui-core --package platynui-link --package platynui-xpath --package platynui-runtime --package platynui-platform-windows --package platynui-provider-windows-uia --package platynui-provider-jab --package platynui-cli --package platynui-inspector --package platynui-cli-bin --package platynui-inspector-bin"
macos_arm_rust_target := env("PLATYNUI_MACOS_ARM_TARGET", "aarch64-apple-darwin")
macos_rust_packages := "--package platynui-core --package platynui-link --package platynui-xpath --package platynui-runtime --package platynui-platform-macos --package platynui-provider-macos-ax --package platynui-cli --package platynui-inspector --package platynui-cli-bin --package platynui-inspector-bin"
# Built platynui-test-app-egui binary the acceptance suites launch (via PLATYNUI_TEST_APP_BIN).
egui_test_app := justfile_directory() / "target" / "debug" / if os() == "windows" { "platynui-test-app-egui.exe" } else { "platynui-test-app-egui" }
# Built platynui-inspector-rs binary the inspector-picker acceptance suite launches (via PLATYNUI_INSPECTOR_BIN).
inspector_bin := justfile_directory() / "target" / "debug" / if os() == "windows" { "platynui-inspector-rs.exe" } else { "platynui-inspector-rs" }
# Qt test app on Windows (handed over via PLATYNUI_TEST_APP_QT_*). Paths must be ABSOLUTE — a relative
# interpreter fails CreateProcess with FileNotFoundError. The project venv holds PySide6 (from the
# build-native sync), but its python.exe is a uv TRAMPOLINE: launching it spawns the real interpreter as a
# CHILD with a different PID, which breaks the tests' @ProcessId window-pinning (and orphans the child on
# teardown). The recipe below therefore launches the BASE interpreter directly and redirects it into the venv
# via __PYVENV_LAUNCHER__ (the mechanism the trampoline itself uses), so the launched PID owns the window.
qt_venv_python := justfile_directory() / ".venv" / "Scripts" / "python.exe"
qt_app_main := justfile_directory() / "apps" / "test-app-qt" / "main.py"
# QML (Qt Quick) test app — same launch mechanics as the Qt Widgets app (base
# interpreter + __PYVENV_LAUNCHER__ redirect, handed over via PLATYNUI_TEST_APP_QML_*).
qml_app_main := justfile_directory() / "apps" / "test-app-qml" / "main.py"
# Swing test app (self-contained Gradle project — see apps/test-app-swing/README.md).
# The build writes the provisioned JVM paths (java8 = default launch runtime,
# java21 = compile toolchain) to java-launchers.properties; the run/acceptance
# recipes resolve the launch JVM from there and fall back to the PATH `java`.
swing_app_dir := justfile_directory() / "apps" / "test-app-swing"
swing_app_classes := swing_app_dir / "build" / "classes" / "java" / "main"
swing_app_launchers := swing_app_dir / "build" / "java-launchers.properties"
# headless runs the Linux acceptance lane with no visible window (compositor uses
# its headless backend, X11 runs under Xvfb). Defaults to true under CI (the
# conventional `CI` env var is set); override anywhere with `just headless=… …`.
headless := if env("CI", "") != "" { "true" } else { "false" }

# When "true", the build recipes that default to debug — `build` (cargo) and the
# maturin-develop installs (build-native, build-cli, build-inspector,
# build-native-mock) — compile in release mode. The *-wheel recipes are always
# release and ignore this. Override per-invocation, e.g. `just release=true build`.
release := "false"

pre_push_cross_command := if os() == "linux" {
    "if [ \"$(git config --bool platynui.pre-push-cross-targets || true)\" != \"true\" ]; then echo \"Skipping optional Linux cross-target checks. Enable with: just hooks-cross-enable\"; else just cross-target-checks; fi"
} else {
    "echo \"Skipping optional Linux cross-target checks on " + os() + ".\""
}

# ─── Default ────────────────────────────────────────────────────────────────────

# List available recipes
default:
    @just --list

# ─── Bootstrap ──────────────────────────────────────────────────────────────────

# Bootstrap the full development environment
bootstrap:
    uv sync --dev --all-packages --all-groups --all-extras --no-install-workspace

# ─── Build ──────────────────────────────────────────────────────────────────────

# Build all Rust crates
build:
    cargo build --workspace --all-targets {{ if release == "true" { "--release" } else { "" } }}

# Build native Python package (with optional features)
build-native *FEATURES:
    uv run maturin develop -m packages/native/Cargo.toml --uv {{ if release == "true" { "--release" } else { "" } }} {{ if FEATURES != "" { "--features " + FEATURES } else { "" } }}

# Build release wheel for native Python package
build-native-wheel *ARGS:
    uv run --no-sync maturin build --release -m packages/native/Cargo.toml -o dist {{ ARGS }}

# Build CLI Python package
build-cli:
    uv run maturin develop -m packages/cli/Cargo.toml --uv {{ if release == "true" { "--release" } else { "" } }}

# Build release wheel for CLI Python package
build-cli-wheel *ARGS:
    uv run --no-sync maturin build --release -m packages/cli/Cargo.toml -o dist {{ ARGS }}

# Build Inspector Python package
build-inspector:
    uv run maturin develop -m packages/inspector/Cargo.toml --uv {{ if release == "true" { "--release" } else { "" } }}

# Build release wheel for Inspector Python package
build-inspector-wheel *ARGS:
    uv run --no-sync maturin build --release -m packages/inspector/Cargo.toml -o dist {{ ARGS }}

# Build release wheel for robotframework-PlatynUI package
build-platynui-wheel:
    uv build --wheel -o dist

# Build all Python packages (native + CLI + Inspector)
build-all-python: build-native build-cli build-inspector

# Build all release wheels
build-all-wheels: build-platynui-wheel build-native-wheel build-cli-wheel build-inspector-wheel

# Build native Python package with mock-provider feature
build-native-mock:
    uv run maturin develop -m packages/native/Cargo.toml --uv {{ if release == "true" { "--release" } else { "" } }} --features mock-provider

# Remove local build and test artifacts, keeping .venv and tool caches
clean:
    cargo clean
    git clean -fdX -- build dist wheelhouse wheels results .pytest_cache ':(glob)packages/*/target' ':(glob)packages/*/build' ':(glob)packages/*/dist'

# ─── Documentation ──────────────────────────────────────────────────────────────

# Build Rust API documentation
doc:
    cargo doc --workspace --no-deps --exclude platynui-cli-bin --exclude platynui-inspector-bin

# ─── Check ──────────────────────────────────────────────────────────────────────

# Format all Rust code
fmt:
    cargo fmt --all

# Check Rust formatting without applying changes
fmt-check:
    cargo fmt --all -- --check

# Run clippy with strict warnings
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Type-check Windows-relevant Rust crates for Windows from Linux
[linux]
check-windows: _check-windows-cross-tools
    cargo check --all-targets --target {{ windows_rust_target }} {{ windows_rust_packages }}

# Type-check macOS ARM-relevant Rust crates from Linux
[linux]
check-macos-arm: _check-macos-cross-tools
    cargo check --all-targets --target {{ macos_arm_rust_target }} {{ macos_rust_packages }}

# Run clippy for Windows-relevant Rust crates from Linux
[linux]
clippy-windows: _check-windows-cross-tools
    cargo clippy --all-targets --target {{ windows_rust_target }} {{ windows_rust_packages }} -- -D warnings

# Run clippy for macOS ARM-relevant Rust crates from Linux
[linux]
clippy-macos-arm: _check-macos-cross-tools
    cargo clippy --all-targets --target {{ macos_arm_rust_target }} {{ macos_rust_packages }} -- -D warnings

# Lint Python code
ruff:
    uv run ruff check

# Type-check Python code (scope comes from [tool.mypy] files in pyproject.toml).
# The Python fixture apps are standalone PEP 723 scripts that all share the module
# name "main", so each needs its own mypy invocation (see the note in pyproject.toml).
mypy:
    uv run mypy
    uv run mypy --no-warn-unused-configs apps/test-app-qt/main.py
    uv run mypy --no-warn-unused-configs apps/test-app-qml/main.py

# Run all checks (format, clippy, ruff, mypy)
check: fmt clippy ruff mypy
    @echo "All checks passed."

# ─── Git Hooks ─────────────────────────────────────────────────────────────────

# Install pre-commit, commit-msg, and pre-push hooks
hooks-install:
    uv run pre-commit install --install-hooks

# Alias for explicit push-gate setup
hooks-install-push: hooks-install
    @echo "pre-push is installed by hooks-install."

# Run pre-commit stage hooks against all files
hooks-run:
    uv run pre-commit run --all-files

# Run the pre-push hook manually
hooks-run-push:
    uv run pre-commit run --hook-stage pre-push --all-files

# Run Linux cross-target checks manually
[linux]
hooks-run-cross:
    just cross-target-checks

# Enable optional Linux cross-target checks before each push
hooks-cross-enable:
    git config platynui.pre-push-cross-targets true
    @echo "Optional Linux cross-target pre-push checks enabled."

# Disable optional Linux cross-target checks before each push
hooks-cross-disable:
    git config --unset platynui.pre-push-cross-targets || true
    @echo "Optional Linux cross-target pre-push checks disabled."

# Run optional Linux cross-target checks from the pre-push hook when enabled
hooks-pre-push-cross:
    @{{ pre_push_cross_command }}

# Update remote pre-commit hook revisions
hooks-update:
    uv run pre-commit autoupdate

# Remove installed git hooks managed by pre-commit
hooks-uninstall:
    uv run pre-commit uninstall --hook-type pre-commit
    uv run pre-commit uninstall --hook-type commit-msg
    uv run pre-commit uninstall --hook-type pre-push

# ─── Test ───────────────────────────────────────────────────────────────────────

# Run all Rust tests
test:
    cargo nextest run --workspace --no-fail-fast

# Run tests for a specific crate
test-crate crate:
    cargo nextest run -p {{ crate }} --no-fail-fast

# Run Python tests (builds native package with mock-provider first)
test-python: build-native-mock
    uv run pytest -v --tb=short --maxfail=3

# Run the mock-backed BareMetal Robot Framework suites via the `mock` profile (tag
# `mock`, paths tests/BareMetal). They use the built-in mock tree (use_mock=True),
# so they need no display and run on any OS; builds the mock-provider native module
# first. ARGS pass through to robotcode.
test-baremetal *ARGS: build-native-mock
    uv run robotcode --profile mock run {{ ARGS }}

# Print a Markdown summary of the most recent Robot Framework run (results/output.xml).
# Reads only the output file (no build needed); feed a CI step summary with
# `just test-summary >> "$GITHUB_STEP_SUMMARY"`. ARGS pass through to robotcode results.
test-summary *ARGS:
    @uv run robotcode results summary --failed {{ ARGS }}

# Run all tests (Rust + Python)
test-all: test test-python

# ─── Acceptance (real desktop, needs the non-mock build) ──────────────────────────
#
# The acceptance lane (tests/acceptance, tag `real`, profile `real`) drives the real
# platform provider against the test apps there (egui today). Each recipe rebuilds the non-mock
# native module first (a mock-provider build would silently resolve the built-in
# mock tree instead). Robot Framework launches the app instance(s) itself; extra
# ARGS are forwarded to robotcode and default to `--profile real run`.
#
# headless defaults to true under CI (the `CI` env var) and runs the Linux backends
# with no visible window — the compositor uses its headless backend and X11 runs
# under Xvfb (needs a GPU render node or Mesa software GL so egui can render). Force
# it anywhere with `just headless=true …`, or disable in CI with `headless=false`.

# Run the egui acceptance lane on this OS (Linux: compositor + X11). Honors headless.
[linux]
test-acceptance: test-acceptance-compositor test-acceptance-x11

# Run the egui acceptance lane on this OS (Windows: the real desktop).
[windows]
test-acceptance: test-acceptance-windows

# Run the egui acceptance lane under the PlatynUI Wayland compositor.
[linux]
test-acceptance-compositor *ARGS: build-native
    uv run scripts/startcompositor.sh {{ if headless == "true" { "--backend headless" } else { "" } }} -- scripts/platynui-robot-session.sh {{ ARGS }}

# Run the egui acceptance lane under an isolated X11 session (Xephyr; Xvfb when headless).
[linux]
test-acceptance-x11 *ARGS: build-native
    uv run scripts/startxsession.sh {{ if headless == "true" { "--backend headless" } else { "" } }} -- scripts/platynui-robot-session.sh {{ ARGS }}

# Run the acceptance lane on the native Windows desktop (UIA + JAB providers).
# No isolated session — the suites launch the apps on the real desktop. Builds
# the egui test app AND the Inspector binary (the inspector-picker suite
# launches it); PySide6 (a dev dependency) is already in the project venv from
# the build-native sync. All fixtures are handed over via the
# PLATYNUI_TEST_APP_* / PLATYNUI_INSPECTOR_BIN env vars (Robot Framework
# launches them). The Swing fixture builds via its Gradle wrapper: if that
# build fails (typically no network on the first run), it fails softly here and
# the swing suites (plus the JAB live Rust tests) skip with a clear message
# instead of failing the lane. The fixture runs on the provisioned Java 8
# runtime (PLATYNUI_TEST_APP_SWING_JAVA, from java-launchers.properties).
[windows]
test-acceptance-windows *ARGS: build-native
    cargo build -p platynui-test-app-egui -p platynui-inspector
    -just build-test-app-swing
    if (Test-Path "{{ swing_app_classes }}") { $env:PLATYNUI_TEST_APP_SWING_CLASSES = "{{ swing_app_classes }}"; if (Test-Path "{{ swing_app_launchers }}") { $env:PLATYNUI_TEST_APP_SWING_JAVA = ((Get-Content -Raw "{{ swing_app_launchers }}") | ConvertFrom-StringData).java8 }; cargo nextest run -p platynui-provider-jab --run-ignored ignored-only } else { Write-Warning "Swing fixture not built (Gradle build failed - no network on first run?) - skipping the JAB live tests" }
    $qtBasePy = & "{{ qt_venv_python }}" -c "import sys; print(sys._base_executable)"; $env:PLATYNUI_TEST_APP_BIN = "{{ egui_test_app }}"; $env:PLATYNUI_INSPECTOR_BIN = "{{ inspector_bin }}"; $env:PLATYNUI_TEST_APP_QT_PYTHON = $qtBasePy; $env:PLATYNUI_TEST_APP_QT_PYVENV_LAUNCHER = "{{ qt_venv_python }}"; $env:PLATYNUI_TEST_APP_QT_MAIN = "{{ qt_app_main }}"; $env:PLATYNUI_TEST_APP_QML_PYTHON = $qtBasePy; $env:PLATYNUI_TEST_APP_QML_PYVENV_LAUNCHER = "{{ qt_venv_python }}"; $env:PLATYNUI_TEST_APP_QML_MAIN = "{{ qml_app_main }}"; $env:PLATYNUI_TEST_APP_SWING_CLASSES = "{{ swing_app_classes }}"; if (Test-Path "{{ swing_app_launchers }}") { $env:PLATYNUI_TEST_APP_SWING_JAVA = ((Get-Content -Raw "{{ swing_app_launchers }}") | ConvertFrom-StringData).java8 }; uv run --no-sync robotcode {{ if ARGS != "" { ARGS } else { "--profile real run" } }}

# Run the QML (Qt Quick) test app on the project venv (PySide6 is a dev
# dependency, installed by `uv sync`). Extra args are forwarded to the app.
run-test-app-qml *ARGS:
    uv run python apps/test-app-qml/main.py {{ ARGS }}

# ─── Swing Test App (Java fixture for the JAB provider work) ───────────────────

# Build the Swing test app via its Gradle wrapper (needs only a `java` 8+ on
# PATH: the Gradle daemon JVM, the JDK 21 compile toolchain and the Java 8
# launch runtime are all auto-provisioned — network access required on the
# first build).
[unix]
build-test-app-swing:
    cd "{{ swing_app_dir }}" && ./gradlew --console=plain classes writeJavaLaunchers

# Build the Swing test app via its Gradle wrapper (needs only a `java` 8+ on
# PATH: the Gradle daemon JVM, the JDK 21 compile toolchain and the Java 8
# launch runtime are all auto-provisioned — network access required on the
# first build).
[windows]
build-test-app-swing:
    Set-Location "{{ swing_app_dir }}"; & .\gradlew.bat --console=plain classes writeJavaLaunchers; exit $LASTEXITCODE

# Run the Swing test app (Gradle `run` task: provisioned Java 8 runtime; on
# Windows the Java Access Bridge is enabled for this process only — no
# jabswitch, no persistent config; Linux ATK-wrapper enablement is documented
# in the app README). ARGS mirror the Qt/egui apps.
[windows]
run-test-app-swing *ARGS:
    Set-Location "{{ swing_app_dir }}"; & .\gradlew.bat --console=plain run {{ if ARGS != "" { '"--args=' + ARGS + '"' } else { "" } }}; exit $LASTEXITCODE

# Run the Swing test app (Gradle `run` task: provisioned Java 8 runtime; on
# Windows the Java Access Bridge is enabled for this process only — no
# jabswitch, no persistent config; Linux ATK-wrapper enablement is documented
# in the app README). ARGS mirror the Qt/egui apps.
[unix]
run-test-app-swing *ARGS:
    cd "{{ swing_app_dir }}" && ./gradlew --console=plain run {{ if ARGS != "" { "--args='" + ARGS + "'" } else { "" } }}

# ─── Desktop Integration ────────────────────────────────────────────────────────

# Install .desktop files and icons into XDG directories
[linux]
install-desktop: (_install-desktop-files) (_install-icons)
    @echo "Desktop files and icons installed to {{ xdg_data_home }}"
    @echo "Run 'just update-icon-cache' if icons don't appear immediately."

# Uninstall .desktop files and icons from XDG directories
[linux]
uninstall-desktop: (_uninstall-desktop-files) (_uninstall-icons)
    @echo "Desktop files and icons removed from {{ xdg_data_home }}"

# Update the icon cache (run after install/uninstall)
[linux]
update-icon-cache:
    gtk-update-icon-cache -f -t "{{ xdg_data_home }}/icons/hicolor" 2>/dev/null || true

[linux]
_install-desktop-files:
    install -Dm644 assets/org.platynui.compositor.desktop "{{ xdg_data_home }}/applications/org.platynui.compositor.desktop"
    install -Dm644 assets/org.platynui.inspector.desktop  "{{ xdg_data_home }}/applications/org.platynui.inspector.desktop"

[linux]
_install-icons:
    install -Dm644 apps/wayland-compositor/assets/icon.png "{{ xdg_data_home }}/icons/hicolor/256x256/apps/org.platynui.compositor.png"
    install -Dm644 apps/inspector/assets/icon.png          "{{ xdg_data_home }}/icons/hicolor/256x256/apps/org.platynui.inspector.png"

[linux]
_uninstall-desktop-files:
    rm -f "{{ xdg_data_home }}/applications/org.platynui.compositor.desktop"
    rm -f "{{ xdg_data_home }}/applications/org.platynui.inspector.desktop"

[linux]
_uninstall-icons:
    rm -f "{{ xdg_data_home }}/icons/hicolor/256x256/apps/org.platynui.compositor.png"
    rm -f "{{ xdg_data_home }}/icons/hicolor/256x256/apps/org.platynui.inspector.png"

# Verify Linux -> Windows GNU cross-compilation prerequisites
[linux]
_check-windows-cross-tools:
    rustup target list --installed | grep -qx '{{ windows_rust_target }}' || \
        (echo 'Missing Rust target {{ windows_rust_target }}. Run: rustup target add {{ windows_rust_target }}' >&2; exit 1)
    command -v x86_64-w64-mingw32-gcc >/dev/null || \
        (echo 'Missing MinGW cross-compiler x86_64-w64-mingw32-gcc. Install mingw-w64-gcc.' >&2; exit 1)
    command -v llvm-rc >/dev/null || \
        (echo 'Missing llvm-rc. Install llvm.' >&2; exit 1)

# Verify Linux -> macOS ARM cross-check prerequisites
[linux]
_check-macos-cross-tools:
    rustup target list --installed | grep -qx '{{ macos_arm_rust_target }}' || \
        (echo 'Missing Rust target {{ macos_arm_rust_target }}. Run: rustup target add {{ macos_arm_rust_target }}' >&2; exit 1)

# ─── Full CI Sequence ───────────────────────────────────────────────────────────

# Run the full pre-commit check sequence
pre-commit: bootstrap check test-all
    @echo "Pre-commit checks passed. Ready to commit!"

# Run Linux cross-target checks (Windows + macOS ARM)
[linux]
cross-target-checks: check-windows clippy-windows check-macos-arm clippy-macos-arm
    @echo "All cross-target checks passed."

# Run pre-commit checks plus cross-target checks (Windows + macOS ARM)
[linux]
pre-commit-cross: pre-commit cross-target-checks
    @echo "All pre-commit and cross-target checks passed."
