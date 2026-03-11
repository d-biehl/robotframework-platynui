#!/bin/bash
set -u

# Create a private XDG_RUNTIME_DIR so the AT-SPI bus socket is fully
# isolated from the host GNOME Wayland session (otherwise
# at-spi-bus-launcher reuses /run/user/$UID/at-spi/bus_$DISPLAY).
XEPHYR_RUNTIME_DIR=$(mktemp -d "/run/user/$(id -u)/xephyr-session-XXXXXX")
XEPHYR_PID=""

cleanup() {
  [ -n "$XEPHYR_PID" ] && kill "$XEPHYR_PID" 2>/dev/null
  if [ -d "$XEPHYR_RUNTIME_DIR" ]; then
    # gvfsd and xdg-document-portal may have created FUSE mounts inside
    # XDG_RUNTIME_DIR (e.g. gvfs, doc). Unmount them before removing.
    for mnt in "$XEPHYR_RUNTIME_DIR"/*/; do
      mountpoint -q "$mnt" 2>/dev/null && fusermount -u "$mnt" 2>/dev/null
    done
    rm -rf "$XEPHYR_RUNTIME_DIR"
  fi
}
trap cleanup EXIT INT TERM

# Detect WSL — -displayfd does not work reliably under WSL, so use a
# fixed display number instead.
IS_WSL=0
if grep -qi microsoft /proc/version 2>/dev/null; then
  IS_WSL=1
fi

if [ "$IS_WSL" -eq 1 ]; then
  # WSL: use a fixed display number (99) since -displayfd is broken
  DISPLAY_NUM=99
  echo "WSL detected — using fixed display :$DISPLAY_NUM"

  Xephyr ":$DISPLAY_NUM" -ac -screen 1920x1080 -noreset -sw-cursor -dpi 192 &
  XEPHYR_PID=$!
  sleep 1

  if ! kill -0 "$XEPHYR_PID" 2>/dev/null; then
    echo "ERROR: Xephyr failed to start on :$DISPLAY_NUM" >&2
    exit 1
  fi
else
  # Native Linux: let Xephyr pick a free display number via -displayfd.
  # Use a named pipe (FIFO) so that reading blocks until Xephyr writes,
  # avoiding race conditions with regular files and unflushed writes.
  DISPLAYFD_FIFO="$XEPHYR_RUNTIME_DIR/displayfd"
  mkfifo "$DISPLAYFD_FIFO"

  Xephyr -displayfd 3 -ac -screen 1920x1080 -noreset -sw-cursor -dpi 192 \
    3>"$DISPLAYFD_FIFO" &
  XEPHYR_PID=$!

  # Read blocks until Xephyr writes the display number and closes the fd
  if ! read -r -t 10 DISPLAY_NUM < "$DISPLAYFD_FIFO"; then
    echo "ERROR: Xephyr did not report a display number within 10s" >&2
    exit 1
  fi
  rm -f "$DISPLAYFD_FIFO"

  if [ -z "$DISPLAY_NUM" ] || ! kill -0 "$XEPHYR_PID" 2>/dev/null; then
    echo "ERROR: Xephyr failed to start" >&2
    exit 1
  fi
fi

echo "Xephyr running on display :$DISPLAY_NUM (PID $XEPHYR_PID)"

# Isolate from host GNOME Wayland session
unset DBUS_SESSION_BUS_ADDRESS
unset WAYLAND_DISPLAY
unset XAUTHORITY
unset AT_SPI_BUS_ADDRESS
unset QT_IM_MODULE
unset QT_IM_MODULES

XDG_RUNTIME_DIR="$XEPHYR_RUNTIME_DIR" \
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

  # exec openbox-session
  # exec startplasma-x11
  exec icewm-session
'
