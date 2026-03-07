# PlatynUI development task runner
# See docs/development.md for details.

set shell := ["bash", "-euo", "pipefail", "-c"]

# XDG data directory for local installs (only meaningful on Linux)
xdg_data_home := if os() == "linux" { env("XDG_DATA_HOME", env("HOME") / ".local" / "share") } else { "" }

# ─── Default ────────────────────────────────────────────────────────────────────

# List available recipes
default:
    @just --list

# ─── Bootstrap ──────────────────────────────────────────────────────────────────

# Bootstrap the full development environment
bootstrap:
    uv sync --dev --all-packages --all-groups --all-extras

# ─── Build ──────────────────────────────────────────────────────────────────────

# Build all Rust crates
build:
    cargo build --workspace --all-targets

# Build native Python package (with optional features)
build-native *FEATURES:
    uv run maturin develop -m packages/native/Cargo.toml --uv {{ if FEATURES != "" { "--features " + FEATURES } else { "" } }}

# Build CLI Python package
build-cli:
    uv run maturin develop -m packages/cli/Cargo.toml --uv

# Build Inspector Python package
build-inspector:
    uv run maturin develop -m packages/inspector/Cargo.toml --uv

# Build all Python packages (native + CLI + Inspector)
build-all-python: build-native build-cli build-inspector

# Build native Python package with mock-provider feature
build-native-mock:
    uv run maturin develop -m packages/native/Cargo.toml --uv --features mock-provider

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

# Lint Python code
ruff:
    uv run ruff check

# Type-check Python code
mypy:
    uv run mypy .

# Run all checks (format, clippy, ruff)
check: fmt clippy ruff

# ─── Test ───────────────────────────────────────────────────────────────────────

# Run all Rust tests
test:
    cargo nextest run --workspace --no-fail-fast

# Run tests for a specific crate
test-crate crate:
    cargo nextest run -p {{ crate }} --no-fail-fast

# Run Python tests
test-python:
    uv run pytest

# Run all tests (Rust + Python)
test-all: test test-python

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

# ─── Full CI Sequence ───────────────────────────────────────────────────────────

# Run the full pre-commit check sequence
pre-commit: bootstrap fmt build clippy test ruff
    @echo "All checks passed."
