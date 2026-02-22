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

# Let Xephyr pick a free display number automatically via -displayfd.
# Use a named pipe (FIFO) so that reading blocks until Xephyr writes,
# avoiding race conditions with regular files and unflushed writes.
DISPLAYFD_FIFO="$XEPHYR_RUNTIME_DIR/displayfd"
mkfifo "$DISPLAYFD_FIFO"

Xephyr -displayfd 3 -ac -screen 1920x1080 -noreset -sw-cursor -dpi 96 \
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

  # Start AT-SPI bus launcher with --launch-immediately to bypass the
  # gsettings/IsEnabled check (no GNOME settings daemon in this session)
  # and --a11y=1 to force accessibility on.
  /usr/lib/at-spi-bus-launcher --launch-immediately --a11y=1 &

  # Wait until org.a11y.Bus is available on the session bus
  ATSPI_READY=0
  for i in $(seq 1 50); do
    if dbus-send --session --dest=org.a11y.Bus --print-reply \
         /org/a11y/bus org.a11y.Bus.GetAddress >/dev/null 2>&1; then
      echo "AT-SPI bus ready after $((i * 100))ms"
      ATSPI_READY=1
      break
    fi
    sleep 0.1
  done

  if [ "$ATSPI_READY" -eq 0 ]; then
    echo "WARNING: AT-SPI bus did not become ready within 5s" >&2
  fi

  # Extract the AT-SPI accessibility bus address
  AT_SPI_ADDR=$(dbus-send --session --dest=org.a11y.Bus --print-reply \
    /org/a11y/bus org.a11y.Bus.GetAddress 2>/dev/null \
    | grep string | head -1 | sed "s/.*\"\(.*\)\"/\1/")

  if [ -n "$AT_SPI_ADDR" ]; then
    echo "AT-SPI accessibility bus at: $AT_SPI_ADDR"

    # Start the registry daemon on the AT-SPI accessibility bus.
    # The inline env var sets DBUS_SESSION_BUS_ADDRESS only for registryd
    # without clobbering the session bus for the rest of the script.
    DBUS_SESSION_BUS_ADDRESS="$AT_SPI_ADDR" /usr/lib/at-spi2-registryd &
    sleep 0.2
    echo "AT-SPI registryd started"
  else
    echo "WARNING: AT-SPI bus not available -- accessibility will not work" >&2
  fi

  setxkbmap de

  # exec openbox-session
  # exec startplasma-x11
  exec icewm-session
'
