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
#   PLATYNUI_LOG_LEVEL     Log level for the compositor (default: debug)
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
    echo "Display detected — using winit backend"
  else
    BACKEND="headless"
    echo "No display detected — using headless backend"
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
  echo "WSL detected — XWayland will be disabled"
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
    echo "Symlinked parent Wayland socket into session runtime dir"
  else
    echo "WARNING: Parent Wayland socket not found at $PARENT_WAYLAND_SOCKET" >&2
    echo "         Falling back to headless backend" >&2
    BACKEND="headless"
  fi
fi

# ---------------------------------------------------------------------------
# Configure xdg-desktop-portal with GTK backend
# ---------------------------------------------------------------------------
mkdir -p "$SESSION_RUNTIME_DIR/xdg-desktop-portal"
cat > "$SESSION_RUNTIME_DIR/xdg-desktop-portal/portals.conf" <<'EOF'
[preferred]
default=gtk
EOF

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

LOG_LEVEL="${PLATYNUI_LOG_LEVEL:-debug}"

echo "=== PlatynUI Compositor Session ==="
echo "Backend:          $BACKEND"
echo "XWayland:         $XWAYLAND"
echo "Runtime dir:      $SESSION_RUNTIME_DIR"
echo "Session command:  ${SESSION_CMD[*]}"
echo "Log level:        $LOG_LEVEL"
echo "===================================="

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

export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=platynui

# Portal configuration
export GTK_USE_PORTAL=1
export XDG_DESKTOP_PORTAL_DIR="\$XDG_RUNTIME_DIR/xdg-desktop-portal"

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

echo "Session XDG_RUNTIME_DIR=\$XDG_RUNTIME_DIR"
echo "Session DBUS_SESSION_BUS_ADDRESS=\$DBUS_SESSION_BUS_ADDRESS"

# ---- AT-SPI accessibility bus setup ----
#
# The at-spi-bus-launcher creates a private dbus-daemon for accessibility.
# Its auto-activation service file uses --use-gnome-session which fails in
# our isolated session.  We override it with a local service file, start the
# bus launcher, then verify the registryd is reachable before proceeding.

# Override the Registry service file to remove --use-gnome-session which
# fails in a non-GNOME session.
A11Y_SERVICES_DIR="\$XDG_RUNTIME_DIR/at-spi-services/dbus-1/accessibility-services"
mkdir -p "\$A11Y_SERVICES_DIR"
cat > "\$A11Y_SERVICES_DIR/org.a11y.atspi.Registry.service" <<A11Y_EOF
[D-BUS Service]
Name=org.a11y.atspi.Registry
Exec=/usr/lib/at-spi2-registryd
A11Y_EOF

# Prepend our override directory to XDG_DATA_DIRS so the AT-SPI bus daemon
# finds our service file before the system one.
export XDG_DATA_DIRS="\$XDG_RUNTIME_DIR/at-spi-services:\${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"

# Start AT-SPI bus launcher with --launch-immediately to bypass the
# gsettings/IsEnabled check and --a11y=1 to force accessibility on.
/usr/lib/at-spi-bus-launcher --launch-immediately --a11y=1 &
AT_SPI_LAUNCHER_PID=\$!

# Wait until org.a11y.Bus is available on the session bus
ATSPI_READY=0
for i in \$(seq 1 50); do
  if dbus-send --session --dest=org.a11y.Bus --print-reply \\
       /org/a11y/bus org.a11y.Bus.GetAddress >/dev/null 2>&1; then
    echo "AT-SPI bus launcher ready after \$((i * 100))ms"
    ATSPI_READY=1
    break
  fi
  sleep 0.1
done

if [ "\$ATSPI_READY" -eq 0 ]; then
  echo "WARNING: AT-SPI bus launcher did not become ready within 5s" >&2
  echo "         at-spi-bus-launcher PID \$AT_SPI_LAUNCHER_PID" >&2
  if ! kill -0 "\$AT_SPI_LAUNCHER_PID" 2>/dev/null; then
    echo "         Process is no longer running!" >&2
  fi
fi

# Extract the AT-SPI accessibility bus address
AT_SPI_ADDR=\$(dbus-send --session --dest=org.a11y.Bus --print-reply \\
  /org/a11y/bus org.a11y.Bus.GetAddress 2>/dev/null \\
  | grep string | head -1 | sed 's/.*"\(.*\)"/\1/')

if [ -n "\$AT_SPI_ADDR" ]; then
  echo "AT-SPI accessibility bus at: \$AT_SPI_ADDR"
  export AT_SPI_BUS_ADDRESS="\$AT_SPI_ADDR"

  # Start the registry daemon on the AT-SPI accessibility bus.
  DBUS_SESSION_BUS_ADDRESS="\$AT_SPI_ADDR" /usr/lib/at-spi2-registryd &
  REGISTRYD_PID=\$!

  # Wait until org.a11y.atspi.Registry is actually available on the AT-SPI bus.
  # This replaces the fragile 'sleep 0.2' and prevents race conditions where
  # clients connect before the registryd has finished initialising.
  REGISTRY_READY=0
  for i in \$(seq 1 50); do
    if DBUS_SESSION_BUS_ADDRESS="\$AT_SPI_ADDR" \\
       dbus-send --session --dest=org.a11y.atspi.Registry --print-reply \\
         /org/a11y/atspi/accessible/root org.freedesktop.DBus.Peer.Ping \\
         >/dev/null 2>&1; then
      echo "AT-SPI registryd ready after \$((i * 100))ms"
      REGISTRY_READY=1
      break
    fi
    sleep 0.1
  done

  if [ "\$REGISTRY_READY" -eq 0 ]; then
    echo "WARNING: AT-SPI registryd did not become ready within 5s" >&2
    if ! kill -0 "\$REGISTRYD_PID" 2>/dev/null; then
      echo "         registryd (PID \$REGISTRYD_PID) is no longer running!" >&2
    fi
  fi
else
  echo "WARNING: AT-SPI bus not available — accessibility will not work" >&2
fi

# Start xdg-desktop-portal with the GTK backend
if command -v xdg-desktop-portal >/dev/null 2>&1; then
  /usr/libexec/xdg-desktop-portal-gtk &
  xdg-desktop-portal &
  echo "xdg-desktop-portal started with GTK backend"
else
  echo "WARNING: xdg-desktop-portal not found — portals unavailable" >&2
fi

# Launch the PlatynUI compositor via cargo run
cd ${PROJECT_DIR@Q}
exec cargo run -p platynui-wayland-compositor -- \\
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
  exec dbus-run-session -- "$INNER_SCRIPT"
