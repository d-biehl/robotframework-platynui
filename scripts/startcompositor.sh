#!/bin/bash
set -u

# Start a fully isolated PlatynUI Wayland compositor session with its own
# D-Bus, AT-SPI accessibility bus, and XDG_RUNTIME_DIR.  The compositor is
# built and launched via `cargo run`.
#
# Usage:
#   scripts/startcompositor.sh [--backend winit|headless] [--xwayland] [-- session-script args...]
#
# If no session script is given after `--`, the default session script
# (scripts/platynui-session.sh) is used.
#
# Environment variables:
#   PLATYNUI_BACKEND       Override backend (default: auto-detect)
#   PLATYNUI_LOG_LEVEL     Log level for the compositor (default: error)
#   PLATYNUI_EXTRA_ARGS    Additional arguments for the compositor

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ---------------------------------------------------------------------------
# Parse arguments: everything before `--` is for this script, everything
# after `--` is the session command.
# ---------------------------------------------------------------------------
BACKEND="${PLATYNUI_BACKEND:-}"
XWAYLAND=0
SESSION_CMD=()
COMPOSITOR_EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --)
      shift
      SESSION_CMD=("$@")
      break
      ;;
    --backend)
      BACKEND="$2"
      shift 2
      ;;
    --backend=*)
      BACKEND="${1#--backend=}"
      shift
      ;;
    --xwayland)
      XWAYLAND=1
      shift
      ;;
    *)
      COMPOSITOR_EXTRA_ARGS+=("$1")
      shift
      ;;
  esac
done

# Default session script
if [[ ${#SESSION_CMD[@]} -eq 0 ]]; then
  SESSION_CMD=("$SCRIPT_DIR/platynui-session.sh")
fi

# ---------------------------------------------------------------------------
# Backend auto-detection
# ---------------------------------------------------------------------------
if [[ -z "$BACKEND" ]]; then
  if [[ -n "${WAYLAND_DISPLAY:-}" ]] || [[ -n "${DISPLAY:-}" ]]; then
    BACKEND="winit"
    echo "Display detected — using winit backend" >&2
  else
    BACKEND="headless"
    echo "No display detected — using headless backend" >&2
  fi
fi

# ---------------------------------------------------------------------------
# Isolated XDG_RUNTIME_DIR
# ---------------------------------------------------------------------------
SESSION_RUNTIME_DIR=$(mktemp -d "/run/user/$(id -u)/platynui-session-XXXXXX")

cleanup() {
  if [[ -d "$SESSION_RUNTIME_DIR" ]]; then
    # Unmount any FUSE mounts (gvfsd, xdg-document-portal) before removal.
    for mnt in "$SESSION_RUNTIME_DIR"/*/; do
      mountpoint -q "$mnt" 2>/dev/null && fusermount -u "$mnt" 2>/dev/null
    done
    rm -rf "$SESSION_RUNTIME_DIR"
  fi
}
trap cleanup EXIT INT TERM

# Under WSL, XWayland does not work reliably.
if grep -qi microsoft /proc/version 2>/dev/null; then
  echo "WSL detected — XWayland will be disabled" >&2
  XWAYLAND=0
fi

# ---------------------------------------------------------------------------
# Symlink host Wayland socket into the isolated runtime dir (for nesting)
# ---------------------------------------------------------------------------
if [[ "$BACKEND" == "winit" ]] && [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
  PARENT_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
  PARENT_WAYLAND_SOCKET="$PARENT_RUNTIME_DIR/$WAYLAND_DISPLAY"
  if [[ -e "$PARENT_WAYLAND_SOCKET" ]]; then
    ln -sf "$PARENT_WAYLAND_SOCKET" "$SESSION_RUNTIME_DIR/$WAYLAND_DISPLAY"
    [[ -e "${PARENT_WAYLAND_SOCKET}.lock" ]] && \
      ln -sf "${PARENT_WAYLAND_SOCKET}.lock" "$SESSION_RUNTIME_DIR/${WAYLAND_DISPLAY}.lock"
    echo "Symlinked parent Wayland socket into session runtime dir" >&2
  else
    echo "WARNING: Parent Wayland socket not found at $PARENT_WAYLAND_SOCKET" >&2
    echo "         Falling back to headless backend" >&2
    BACKEND="headless"
  fi
fi

# ---------------------------------------------------------------------------
# Build compositor args
# ---------------------------------------------------------------------------
COMPOSITOR_ARGS=(
  --backend "$BACKEND"
  --print-env
  --keyboard-layout de
  --software-cursor
  --exit-with-child
)

if [[ "$XWAYLAND" -eq 1 ]]; then
  COMPOSITOR_ARGS+=(--xwayland)
fi

# Pass any extra args from PLATYNUI_EXTRA_ARGS env or CLI
if [[ -n "${PLATYNUI_EXTRA_ARGS:-}" ]]; then
  # shellcheck disable=SC2206
  COMPOSITOR_ARGS+=($PLATYNUI_EXTRA_ARGS)
fi
COMPOSITOR_ARGS+=("${COMPOSITOR_EXTRA_ARGS[@]}")

# Session script goes after `--`
COMPOSITOR_ARGS+=(--)
COMPOSITOR_ARGS+=("${SESSION_CMD[@]}")

LOG_LEVEL="${PLATYNUI_LOG_LEVEL:-error}"

echo "=== PlatynUI Compositor Session ===" >&2
echo "Backend:          $BACKEND" >&2
echo "XWayland:         $XWAYLAND" >&2
echo "Runtime dir:      $SESSION_RUNTIME_DIR" >&2
echo "Session command:  ${SESSION_CMD[*]}" >&2
echo "Log level:        $LOG_LEVEL" >&2
echo "====================================" >&2

# ---------------------------------------------------------------------------
# Write the inner session bootstrap script to a temp file.
# This avoids fragile quoting inside `bash -c '...'`.
# ---------------------------------------------------------------------------
INNER_SCRIPT="$SESSION_RUNTIME_DIR/inner-session.sh"

# Serialize the COMPOSITOR_ARGS array into a safe shell representation
SERIALIZED_ARGS="$(printf '%q ' "${COMPOSITOR_ARGS[@]}")"

cat > "$INNER_SCRIPT" <<INNER_EOF
#!/bin/bash
set -u

# Track background PIDs for cleanup
BG_PIDS=()

cleanup_inner() {
  for pid in "\${BG_PIDS[@]}"; do
    kill "\$pid" 2>/dev/null
  done
  wait 2>/dev/null
}
trap cleanup_inner EXIT INT TERM

export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=platynui

# Accessibility environment
export NO_AT_BRIDGE=0
export ACCESSIBILITY_ENABLED=1
export GTK_A11Y=atspi
export QT_ACCESSIBILITY=1
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export GDK_BACKEND=wayland
export QT_QPA_PLATFORM=wayland

export LANG=de_DE.UTF-8
export LC_ALL=de_DE.UTF-8

echo "Session XDG_RUNTIME_DIR=\$XDG_RUNTIME_DIR" >&2
echo "Session DBUS_SESSION_BUS_ADDRESS=\$DBUS_SESSION_BUS_ADDRESS" >&2

# Launch the PlatynUI compositor via cargo run
cd ${PROJECT_DIR@Q}
cargo run -p platynui-wayland-compositor -- \\
  --log-level ${LOG_LEVEL@Q} \\
  $SERIALIZED_ARGS
INNER_EOF
chmod +x "$INNER_SCRIPT"

# ---------------------------------------------------------------------------
# Launch inside an isolated D-Bus session
# ---------------------------------------------------------------------------
# Isolate from the host session
unset DBUS_SESSION_BUS_ADDRESS
unset AT_SPI_BUS_ADDRESS
unset QT_IM_MODULE
unset QT_IM_MODULES

XDG_RUNTIME_DIR="$SESSION_RUNTIME_DIR" \
  dbus-run-session -- "$INNER_SCRIPT"
