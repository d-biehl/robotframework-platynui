#!/bin/bash
set -u

# Options (before `--`):
#   --backend auto|nested|headless   Display server (default: auto). `nested` runs
#       Xephyr inside the host X display (a visible window — for local dev);
#       `headless` runs a standalone Xvfb (no window — for CI); `auto` picks nested
#       when a DISPLAY is present, else headless. Shares the --backend vocabulary
#       with startcompositor.sh (winit/xephyr are accepted as aliases for nested).
#   --dpi <n>     Xephyr DPI (nested backend only; default: unset).
# Session command: everything after `--` is run inside the session, with the
# window manager started in the background. With no `--`, the script keeps its
# original behaviour and execs the interactive window manager.
#
#   uv run scripts/startxsession.sh -- scripts/platynui-robot-session.sh                     # auto (nested if a display is present)
#   uv run scripts/startxsession.sh --backend headless -- scripts/platynui-robot-session.sh  # CI / no display
#
# Environment variables:
#   PLATYNUI_BACKEND   Override backend (default: auto-detect), same as startcompositor.sh.
#
SESSION_CMD=()
DPI=""
BACKEND="${PLATYNUI_BACKEND:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --) shift; SESSION_CMD=("$@"); break ;;
    --backend) BACKEND="$2"; shift 2 ;;
    --backend=*) BACKEND="${1#--backend=}"; shift ;;
    --dpi) DPI="$2"; shift 2 ;;
    --dpi=*) DPI="${1#--dpi=}"; shift ;;
    *) shift ;;
  esac
done

# Resolve the display backend (vocabulary shared with startcompositor.sh):
#   auto (default) → nested if a DISPLAY is present, else headless
#   nested         → Xephyr nested in the host X display (visible; local dev)
#   headless       → standalone Xvfb (no window; CI). winit/xephyr alias nested.
case "$BACKEND" in
  ""|auto) if [ -n "${DISPLAY:-}" ]; then BACKEND=nested; else BACKEND=headless; fi ;;
  nested|xephyr|winit) BACKEND=nested ;;
  headless) BACKEND=headless ;;
  *) echo "ERROR: unknown --backend '$BACKEND' (use auto|nested|headless)" >&2; exit 1 ;;
esac
echo "X11 session backend: $BACKEND" >&2
# Serialize the session command and pass it to the inner session shell via an
# exported variable (avoids fragile quote-breakout inside the bash -c body).
PLATYNUI_ROBOT_SESSION_CMD=""
if [[ ${#SESSION_CMD[@]} -gt 0 ]]; then
  PLATYNUI_ROBOT_SESSION_CMD="$(printf '%q ' "${SESSION_CMD[@]}")"
fi
export PLATYNUI_ROBOT_SESSION_CMD

# Optional Xephyr args. DPI is off by default (configurable via --dpi).
XEPHYR_EXTRA=()
if [[ -n "$DPI" ]]; then
  XEPHYR_EXTRA+=(-dpi "$DPI")
fi

# Create a private XDG_RUNTIME_DIR so the AT-SPI bus socket is fully
# isolated from the host GNOME Wayland session (otherwise
# at-spi-bus-launcher reuses /run/user/$UID/at-spi/bus_$DISPLAY).
SESSION_RUNTIME_DIR=$(mktemp -d "/run/user/$(id -u)/xephyr-session-XXXXXX")
XSERVER_PID=""

cleanup() {
  [ -n "$XSERVER_PID" ] && kill "$XSERVER_PID" 2>/dev/null
  if [ -d "$SESSION_RUNTIME_DIR" ]; then
    # gvfsd and xdg-document-portal may have created FUSE mounts inside
    # XDG_RUNTIME_DIR (e.g. gvfs, doc). Unmount them before removing.
    for mnt in "$SESSION_RUNTIME_DIR"/*/; do
      mountpoint -q "$mnt" 2>/dev/null && fusermount -u "$mnt" 2>/dev/null
    done
    rm -rf "$SESSION_RUNTIME_DIR"
  fi
}
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# Start the X server for the session and capture its display number.
#   headless → standalone Xvfb (own framebuffer, no parent display; -ac lets the
#              isolated session's clients connect without an xauth cookie, which
#              is what makes AT-SPI work headless on CI).
#   nested   → Xephyr inside the host X display (a visible window for local dev).
# ---------------------------------------------------------------------------
DISPLAYFD_FIFO="$SESSION_RUNTIME_DIR/displayfd"
DISPLAY_NUM=""

if [ "$BACKEND" = "headless" ]; then
  echo "Starting Xvfb (headless) ..." >&2
  mkfifo "$DISPLAYFD_FIFO"
  Xvfb -displayfd 3 -screen 0 1920x1080x24 -ac -nolisten tcp 3>"$DISPLAYFD_FIFO" &
  XSERVER_PID=$!
elif grep -qi microsoft /proc/version 2>/dev/null; then
  # WSL: Xephyr's -displayfd is unreliable, so use a fixed display number.
  DISPLAY_NUM=99
  echo "WSL detected — Xephyr on fixed display :$DISPLAY_NUM" >&2
  Xephyr ":$DISPLAY_NUM" -ac -screen 1920x1080 -resizeable -noreset -sw-cursor "${XEPHYR_EXTRA[@]}" &
  XSERVER_PID=$!
  sleep 1
else
  echo "Starting Xephyr (nested) ..." >&2
  mkfifo "$DISPLAYFD_FIFO"
  # Named pipe so the read blocks until Xephyr writes the display number and
  # closes the fd (avoids races with regular files / unflushed writes).
  Xephyr -displayfd 3 -ac -screen 1920x1080 -resizeable -noreset -sw-cursor "${XEPHYR_EXTRA[@]}" \
    3>"$DISPLAYFD_FIFO" &
  XSERVER_PID=$!
fi

# For the -displayfd backends, block until the server reports its display number.
if [ -z "$DISPLAY_NUM" ]; then
  if ! read -r -t 10 DISPLAY_NUM < "$DISPLAYFD_FIFO"; then
    echo "ERROR: X server ($BACKEND) did not report a display number within 10s" >&2
    exit 1
  fi
  rm -f "$DISPLAYFD_FIFO"
fi

if [ -z "$DISPLAY_NUM" ] || ! kill -0 "$XSERVER_PID" 2>/dev/null; then
  echo "ERROR: X server ($BACKEND) failed to start" >&2
  exit 1
fi

echo "X server running on display :$DISPLAY_NUM (PID $XSERVER_PID, backend $BACKEND)"

# Isolate from host GNOME Wayland session
unset DBUS_SESSION_BUS_ADDRESS
unset WAYLAND_DISPLAY
unset XAUTHORITY
unset AT_SPI_BUS_ADDRESS
unset QT_IM_MODULE
unset QT_IM_MODULES

XDG_RUNTIME_DIR="$SESSION_RUNTIME_DIR" \
dbus-run-session -- bash -c '
  export DISPLAY=:'"$DISPLAY_NUM"'
  export XDG_SESSION_TYPE=x11
  export XDG_CURRENT_DESKTOP=openbox

  # Accessibility environment
  export NO_AT_BRIDGE=0
  export ACCESSIBILITY_ENABLED=1
  export GTK_A11Y=atspi
  export QT_ACCESSIBILITY=1
  export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
  export GDK_BACKEND=x11

  export LANG=de_DE.UTF-8
  export LC_ALL=de_DE.UTF-8

  echo "Session DISPLAY=$DISPLAY"
  echo "Session XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR"
  echo "Session DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS"

  # ---- AT-SPI accessibility bus setup ----
  #
  # The at-spi-bus-launcher creates a private dbus-daemon for accessibility.
  # Its auto-activation service file uses --use-gnome-session which fails in
  # our isolated session.  We override it with a local service file.

  # Override the Registry service file to remove --use-gnome-session
  A11Y_SERVICES_DIR="$XDG_RUNTIME_DIR/at-spi-services/dbus-1/accessibility-services"
  mkdir -p "$A11Y_SERVICES_DIR"
  cat > "$A11Y_SERVICES_DIR/org.a11y.atspi.Registry.service" <<A11Y_EOF
[D-BUS Service]
Name=org.a11y.atspi.Registry
Exec=/usr/lib/at-spi2-registryd
A11Y_EOF

  # Prepend our override directory to XDG_DATA_DIRS so the AT-SPI bus daemon
  # finds our service file before the system one.
  export XDG_DATA_DIRS="$XDG_RUNTIME_DIR/at-spi-services:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"

  # Start AT-SPI bus launcher with --launch-immediately to bypass the
  # gsettings/IsEnabled check (no GNOME settings daemon in this session)
  # and --a11y=1 to force accessibility on.
  /usr/lib/at-spi-bus-launcher --launch-immediately --a11y=1 &
  AT_SPI_LAUNCHER_PID=$!

  # Wait until org.a11y.Bus is available on the session bus
  ATSPI_READY=0
  for i in $(seq 1 50); do
    if dbus-send --session --dest=org.a11y.Bus --print-reply \
         /org/a11y/bus org.a11y.Bus.GetAddress >/dev/null 2>&1; then
      echo "AT-SPI bus launcher ready after $((i * 100))ms"
      ATSPI_READY=1
      break
    fi
    sleep 0.1
  done

  if [ "$ATSPI_READY" -eq 0 ]; then
    echo "WARNING: AT-SPI bus launcher did not become ready within 5s" >&2
    echo "         at-spi-bus-launcher PID $AT_SPI_LAUNCHER_PID" >&2
    if ! kill -0 "$AT_SPI_LAUNCHER_PID" 2>/dev/null; then
      echo "         Process is no longer running!" >&2
    fi
  fi

  # Extract the AT-SPI accessibility bus address
  AT_SPI_ADDR=$(dbus-send --session --dest=org.a11y.Bus --print-reply \
    /org/a11y/bus org.a11y.Bus.GetAddress 2>/dev/null \
    | grep string | head -1 | sed "s/.*\"\(.*\)\"/\1/")

  if [ -n "$AT_SPI_ADDR" ]; then
    echo "AT-SPI accessibility bus at: $AT_SPI_ADDR"
    export AT_SPI_BUS_ADDRESS="$AT_SPI_ADDR"

    # Start the registry daemon on the AT-SPI accessibility bus.
    DBUS_SESSION_BUS_ADDRESS="$AT_SPI_ADDR" /usr/lib/at-spi2-registryd &
    REGISTRYD_PID=$!

    # Wait until org.a11y.atspi.Registry is actually available on the AT-SPI bus.
    REGISTRY_READY=0
    for i in $(seq 1 50); do
      if DBUS_SESSION_BUS_ADDRESS="$AT_SPI_ADDR" \
         dbus-send --session --dest=org.a11y.atspi.Registry --print-reply \
           /org/a11y/atspi/accessible/root org.freedesktop.DBus.Peer.Ping \
           >/dev/null 2>&1; then
        echo "AT-SPI registryd ready after $((i * 100))ms"
        REGISTRY_READY=1
        break
      fi
      sleep 0.1
    done

    if [ "$REGISTRY_READY" -eq 0 ]; then
      echo "WARNING: AT-SPI registryd did not become ready within 5s" >&2
      if ! kill -0 "$REGISTRYD_PID" 2>/dev/null; then
        echo "         registryd (PID $REGISTRYD_PID) is no longer running!" >&2
      fi
    fi
  else
    echo "WARNING: AT-SPI bus not available -- accessibility will not work" >&2
  fi

  setxkbmap de

  if [ -n "${PLATYNUI_ROBOT_SESSION_CMD:-}" ]; then
    # Run the window manager in the background, then the session command in the
    # foreground; tear the WM down when the command exits so the outer trap can
    # clean up Xephyr and the runtime dir. The whole session closes once the
    # session command (e.g. the robot run) returns.
    icewm-session &
    WM_PID=$!
    sleep 1
    eval "$PLATYNUI_ROBOT_SESSION_CMD"
    SESSION_EXIT=$?
    kill "$WM_PID" 2>/dev/null
    exit $SESSION_EXIT
  fi

  # exec openbox-session
  # exec startplasma-x11
  exec icewm-session
'
