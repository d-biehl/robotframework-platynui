#!/bin/bash
set -u

# platynui-robot-session.sh — bring up accessibility, launch the egui test
# application, then run RobotCode against it inside the active session.
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
#   robotcode --profile egui run
#
# i.e. the real AT-SPI runtime driving the egui app (see the "egui" profile in
# robot.toml). For interactive debugging — which halts on the first uncaught
# failure and drops into a live (rdb) prompt — pass `run-debug` instead, e.g.
#
#   uv run scripts/startcompositor.sh -- scripts/platynui-robot-session.sh --profile egui run-debug
#
# Environment overrides:
#   PLATYNUI_TEST_APP_TITLE   egui window title (default below)
#   PLATYNUI_TEST_APP_ID      egui app id        (default below)
#   PLATYNUI_APP_GRACE        seconds to wait for the app window (default 2)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

APP_TITLE="${PLATYNUI_TEST_APP_TITLE:-PlatynUI Test App}"
APP_ID="${PLATYNUI_TEST_APP_ID:-com.platynui.test}"
APP_GRACE="${PLATYNUI_APP_GRACE:-2}"

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

cd "$PROJECT_DIR"

# Build up front so the background launch starts immediately instead of racing
# a slow first-run compile against the grace period.
echo "Building platynui-test-app-egui ..." >&2
if ! cargo build -q -p platynui-test-app-egui; then
  echo "ERROR: failed to build platynui-test-app-egui" >&2
  exit 1
fi

APP_BIN="$PROJECT_DIR/target/debug/platynui-test-app-egui"
echo "Launching test app: $APP_BIN --app-id $APP_ID --title '$APP_TITLE'" >&2
"$APP_BIN" --app-id "$APP_ID" --title "$APP_TITLE" &
APP_PID=$!

cleanup() {
  # Only reap the app. A bare `wait` would block on the AT-SPI daemons that
  # setup-atspi.sh started in the background (they never exit), which would
  # hang this script — and with it the compositor (--exit-with-child waits for
  # THIS process). The daemons are torn down with the session afterwards.
  kill "$APP_PID" 2>/dev/null
  wait "$APP_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# Grace period for the app to map its window and publish its AT-SPI tree. The
# RobotCode suite should additionally wait for the window before interacting.
sleep "$APP_GRACE"

if ! kill -0 "$APP_PID" 2>/dev/null; then
  echo "ERROR: test app exited before tests could run" >&2
  exit 1
fi

# Default RobotCode command if none was supplied.
if [ "$#" -eq 0 ]; then
  set -- --profile egui run
fi

echo "Running: robotcode $*" >&2
# --no-sync: the environment is already prepared by the outer `uv run`.
uv run --no-sync robotcode "$@"
