# Wrap a lane session script in a transient systemd user scope (sourced by
# startcompositor.sh and startxsession.sh — not executable on its own).
#
# Why: the lanes spawn a private D-Bus/AT-SPI stack whose daemons
# (at-spi-bus-launcher → dbus-broker, at-spi2-registryd) double-fork out of
# the script's process tree, so PID-based traps cannot reap them — and a
# hard-killed run executes no traps at all. A leftover registryd then poisons
# the next lane run ("window not on the accessibility tree" in every suite).
# Double-forking escapes the process tree but never the cgroup: running the
# script inside a transient scope lets `systemctl --user stop` tear down
# everything the lane started, and only that — the host session's own
# at-spi2-registryd lives in the login session's cgroup and is untouchable
# from here by construction.
#
# Usage, as the first action of a lane script (before any side effects):
#
#   source "$(dirname "$0")/lane-scope.sh"
#   platynui_lane_scope "$0" "$@"
#
# Without a systemd user manager (CI runners, containers) this is a no-op and
# the script behaves exactly as before.

platynui_lane_scope_available() {
  command -v systemd-run >/dev/null 2>&1 || return 1
  systemctl --user show-environment >/dev/null 2>&1 || return 1
}

# Stop scopes left behind by runs that were killed hard (SIGKILL runs no
# cleanup). Units are named platynui-lane-<wrapper pid>.scope; the wrapper
# process lives outside the scope, so a dead wrapper PID marks the scope as
# stale. Only units with our prefix are ever touched.
platynui_lane_scope_sweep() {
  local unit pid
  while read -r unit; do
    [[ "$unit" =~ ^platynui-lane-([0-9]+)\.scope$ ]] || continue
    pid="${BASH_REMATCH[1]}"
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "Sweeping stale lane scope $unit (wrapper $pid is gone)" >&2
      systemctl --user stop "$unit" 2>/dev/null
    fi
  done < <(systemctl --user list-units --all --plain --no-legend 'platynui-lane-*.scope' 2>/dev/null | awk '{print $1}')
}

# Re-run the calling script inside a transient scope and stop the scope when
# it exits. Inside the scope (or without systemd) this is a no-op and the
# caller just continues. `--scope` (rather than a transient service) keeps
# the caller's environment, terminal pipes, and working directory — the
# DISPLAY/WAYLAND_DISPLAY plumbing works unchanged.
platynui_lane_scope() {
  [[ -z "${PLATYNUI_LANE_SCOPE:-}" ]] || return 0
  platynui_lane_scope_available || return 0

  platynui_lane_scope_sweep

  local unit="platynui-lane-$$"
  export PLATYNUI_LANE_SCOPE="$unit"
  echo "Running lane in transient scope $unit.scope" >&2
  # Stop the scope on any exit — normal, INT, or TERM — so every process the
  # lane started is gone afterwards, tracked by a PID variable or not.
  # shellcheck disable=SC2064
  trap "systemctl --user stop '$unit.scope' 2>/dev/null" EXIT INT TERM
  systemd-run --user --scope --collect --quiet --unit="$unit" -- "$@"
  exit $?
}
