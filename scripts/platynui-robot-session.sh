#!/bin/bash
set -u

# platynui-robot-session.sh — bring up accessibility, build the test fixtures
# and hand over to the RobotCode run inside the active session.
#
# This is the last link of a RobotCode `wrapper` chain: the acceptance profiles
# in robot.toml configure
#
#   wrapper = ["scripts/startcompositor.sh", "--", "scripts/platynui-robot-session.sh"]
#   wrapper = ["scripts/startxsession.sh",   "--", "scripts/platynui-robot-session.sh"]
#
# so robotcode re-executes itself through the session scripts and appends its
# own command line — the whole stack (compositor/X server + D-Bus + AT-SPI +
# fixtures + the run) lives and dies together. That appended command arrives
# here as "$@"; this script only prepares the environment and `exec`s it, per
# the wrapper contract (foreground, stdio passed through, exit code propagated).
#
# There is therefore nothing to start here by hand — run the lane by profile:
#
#   uv run robotcode --profile real-wayland run         # PlatynUI compositor
#   uv run robotcode --profile real-x11 run             # X11 (Xephyr, Xvfb when headless)
#   uv run robotcode --profile real-wayland run-debug   # halts on the first failure, (rdb) prompt
#   PLATYNUI_BACKEND=headless uv run robotcode --profile real-x11 run   # no visible window
#
# The profile picks both the session (via its wrapper) and the suites (the
# platform:* tag excludes), so the two can no longer drift apart. Only commands
# that execute Robot Framework are wrapped — discovery, `libdoc` and the
# language server never bring a session up.
#
# Robot Framework launches the app instance(s) itself (see tests/acceptance/),
# so the suites decide how many windows exist and tear them down; this script
# only compiles the binaries and hands their paths over via the environment.
#
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Refuse a bare invocation before touching anything: without the command
# RobotCode appends there is nothing to run, and everything below has side
# effects (accessibility bus, fixture builds).
if [ "$#" -eq 0 ]; then
  echo "ERROR: nothing to run — this script is the tail of a RobotCode wrapper and" >&2
  echo "       is started by robotcode itself, which appends the command to run." >&2
  echo "       Run the lane by profile instead, e.g.:" >&2
  echo "         uv run robotcode --profile real-wayland run" >&2
  exit 2
fi

# Bring up the AT-SPI accessibility bus, but only if the surrounding session
# has not already done so. The compositor session does NOT set up AT-SPI (its
# session script does), whereas startxsession.sh sets AT_SPI_BUS_ADDRESS
# itself — and setup-atspi.sh is not idempotent, so guard against a double
# launch.
if [ -z "${AT_SPI_BUS_ADDRESS:-}" ]; then
  # shellcheck source=scripts/setup-atspi.sh
  source "$SCRIPT_DIR/setup-atspi.sh"
else
  echo "AT-SPI already configured (AT_SPI_BUS_ADDRESS set) — skipping setup-atspi.sh" >&2
fi

# CRITICAL: accesskit_unix only registers its AT-SPI adapter when
# org.a11y.Status.ScreenReaderEnabled is true. Without this the egui app
# (and any AccessKit client) is invisible to the AT-SPI provider — the tree
# resolves a desktop root but no app subtree, which is flaky to debug. Enable
# it on this session's a11y bus before launching the app so AccessKit registers.
"$SCRIPT_DIR/linux-a11y-enable.sh" || echo "WARNING: failed to enable a11y screen-reader status" >&2

cd "$PROJECT_DIR"

# Build the test app up front (a slow first-run compile must not race the
# suite). Robot Framework launches the instance(s) itself and tears them down,
# so we only compile here and hand the binary path over via the environment.
echo "Building platynui-test-app-egui ..." >&2
if ! cargo build -q -p platynui-test-app-egui; then
  echo "ERROR: failed to build platynui-test-app-egui" >&2
  exit 1
fi

export PLATYNUI_TEST_APP_BIN="$PROJECT_DIR/target/debug/platynui-test-app-egui"
echo "Test app binary: $PLATYNUI_TEST_APP_BIN (Robot Framework launches it)" >&2

# The inspector-picker suite launches the real Inspector; build it up front and
# hand its path over the same way (Robot Framework launches it too).
echo "Building platynui-inspector ..." >&2
if ! cargo build -q -p platynui-inspector; then
  echo "ERROR: failed to build platynui-inspector" >&2
  exit 1
fi
export PLATYNUI_INSPECTOR_BIN="$PROJECT_DIR/target/debug/platynui-inspector-rs"
echo "Inspector binary: $PLATYNUI_INSPECTOR_BIN (Robot Framework launches it)" >&2

# Hand the Qt (PySide6) test app's interpreter + entrypoint to Robot Framework.
# PySide6 is a normal dev dependency of the project venv, so we just point at
# the project interpreter. Robot Framework launches
# that Python DIRECTLY — via `uv run` the started PID would differ from the app's
# PID (uv spawns Python as a child), breaking the @ProcessId window pinning.
export PLATYNUI_TEST_APP_QT_PYTHON="$PROJECT_DIR/.venv/bin/python"
export PLATYNUI_TEST_APP_QT_MAIN="$PROJECT_DIR/apps/test-app-qt/main.py"
# The QML (Qt Quick) fixture uses the same launch mechanics (PySide6 from the
# project venv, launched directly so the PID owns the window).
export PLATYNUI_TEST_APP_QML_PYTHON="$PROJECT_DIR/.venv/bin/python"
export PLATYNUI_TEST_APP_QML_MAIN="$PROJECT_DIR/apps/test-app-qml/main.py"
# Qt only exposes its AT-SPI bridge when accessibility is enabled. The
# screen-reader status enabled above covers AccessKit; these cover Qt.
export QT_ACCESSIBILITY=1
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
echo "Qt test app: $PLATYNUI_TEST_APP_QT_PYTHON $PLATYNUI_TEST_APP_QT_MAIN (Robot Framework launches it)" >&2
echo "QML test app: $PLATYNUI_TEST_APP_QML_PYTHON $PLATYNUI_TEST_APP_QML_MAIN (Robot Framework launches it)" >&2

# Run the command RobotCode appended to the wrapper. `exec` replaces this
# script with it, so stdio, signals and the exit code pass through on their own
# — nothing here needs tearing down (the session scripts own the session).
echo "Running: $*" >&2
exec "$@"
