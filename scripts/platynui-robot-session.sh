#!/bin/bash
set -u

# platynui-robot-session.sh — bring up accessibility, build the egui test
# application and run RobotCode inside the active session. Robot Framework
# launches the app instance(s) itself (see tests/acceptance/egui), so the suites
# decide how many windows exist and tear them down; this script only compiles
# the binary and hands its path over via PLATYNUI_TEST_APP_BIN.
#
# Intended as the *session command* for an isolated graphical session, so the
# whole stack (compositor/X server + D-Bus + AT-SPI + app + RobotCode) lives
# and dies together:
#
#   # Wayland (PlatynUI compositor):
#   uv run scripts/startcompositor.sh -- scripts/platynui-robot-session.sh [robotcode-args...]
#
#   # X11 (Xephyr):
#   uv run scripts/startxsession.sh   -- scripts/platynui-robot-session.sh [robotcode-args...]
#
# With no robotcode-args, the default command is:
#
#   robotcode --profile real run
#
# i.e. the real AT-SPI runtime driving the egui app (see the "egui" profile in
# robot.toml). For interactive debugging — which halts on the first uncaught
# failure and drops into a live (rdb) prompt — pass `run-debug` instead, e.g.
#
#   uv run scripts/startcompositor.sh -- scripts/platynui-robot-session.sh --profile real run-debug
#
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

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

# Default RobotCode command if none was supplied.
if [ "$#" -eq 0 ]; then
  set -- --profile real run
fi

echo "Running: robotcode $*" >&2
# --no-sync: the environment is already prepared by the outer `uv run`.
uv run --no-sync robotcode "$@"
