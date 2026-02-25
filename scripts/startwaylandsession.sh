#!/bin/bash
set -u

# Create a private XDG_RUNTIME_DIR so the AT-SPI bus socket is fully
# isolated from the host session (otherwise at-spi-bus-launcher reuses
# /run/user/$UID/at-spi/bus).
WESTON_RUNTIME_DIR=$(mktemp -d "/run/user/$(id -u)/weston-session-XXXXXX")
WESTON_PID=""

cleanup() {
  [ -n "$WESTON_PID" ] && kill "$WESTON_PID" 2>/dev/null
  if [ -d "$WESTON_RUNTIME_DIR" ]; then
    # gvfsd and xdg-document-portal may have created FUSE mounts inside
    # XDG_RUNTIME_DIR (e.g. gvfs, doc). Unmount them before removing.
    for mnt in "$WESTON_RUNTIME_DIR"/*/; do
      mountpoint -q "$mnt" 2>/dev/null && fusermount -u "$mnt" 2>/dev/null
    done
    rm -rf "$WESTON_RUNTIME_DIR"
  fi
}
trap cleanup EXIT INT TERM

# Detect WSL — XWayland does not work reliably under WSL,
# so we disable it there.
IS_WSL=0
if grep -qi microsoft /proc/version 2>/dev/null; then
  IS_WSL=1
  echo "WSL detected — XWayland will be disabled"
fi

# Choose Weston backend depending on environment.
# - If WAYLAND_DISPLAY is set, nest inside the host Wayland compositor.
# - If DISPLAY is set (X11), use the X11 backend.
# - Otherwise fall back to headless (useful for CI).
if [ -n "${WAYLAND_DISPLAY:-}" ]; then
  WESTON_BACKEND="wayland"
  echo "Host Wayland session detected — nesting via wayland backend"

  # The parent compositor's Wayland socket lives in the original
  # XDG_RUNTIME_DIR. Since we use an isolated runtime dir, we must
  # symlink the socket so that Weston can find the parent compositor.
  PARENT_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
  PARENT_WAYLAND_SOCKET="$PARENT_RUNTIME_DIR/$WAYLAND_DISPLAY"
  if [ -e "$PARENT_WAYLAND_SOCKET" ]; then
    ln -sf "$PARENT_WAYLAND_SOCKET" "$WESTON_RUNTIME_DIR/$WAYLAND_DISPLAY"
    # Also symlink the lock file if it exists
    if [ -e "${PARENT_WAYLAND_SOCKET}.lock" ]; then
      ln -sf "${PARENT_WAYLAND_SOCKET}.lock" "$WESTON_RUNTIME_DIR/${WAYLAND_DISPLAY}.lock"
    fi
    echo "Symlinked parent Wayland socket: $PARENT_WAYLAND_SOCKET -> $WESTON_RUNTIME_DIR/$WAYLAND_DISPLAY"
  else
    echo "WARNING: Parent Wayland socket not found at $PARENT_WAYLAND_SOCKET" >&2
    echo "         Falling back to headless backend" >&2
    WESTON_BACKEND="headless"
  fi
elif [ -n "${DISPLAY:-}" ]; then
  WESTON_BACKEND="x11"
  echo "Host X11 session detected — nesting via x11 backend"
else
  WESTON_BACKEND="headless"
  echo "No display detected — using headless backend"
fi

# Weston socket name for the nested session
WESTON_SOCKET="weston-platynui-$$"

# Isolate from host session
unset DBUS_SESSION_BUS_ADDRESS
unset AT_SPI_BUS_ADDRESS
unset QT_IM_MODULE
unset QT_IM_MODULES

# Write a minimal weston.ini for the session.
# Under WSL, XWayland is disabled because it does not start correctly.
WESTON_INI="$WESTON_RUNTIME_DIR/weston.ini"
if [ "$IS_WSL" -eq 1 ]; then
  XWAYLAND_SETTING="false"
else
  XWAYLAND_SETTING="true"
fi

cat > "$WESTON_INI" <<EOF
[core]
xwayland=$XWAYLAND_SETTING

[shell]
panel-position=bottom

[keyboard]
keymap_layout=de

[output]
name=WL-1
mode=1920x1080

[output]
name=X1
mode=1920x1080

[output]
name=headless
mode=1920x1080
EOF

# Configure xdg-desktop-portal to use the GTK backend (the best generic
# portal implementation for non-GNOME/non-KDE compositors like Weston).
mkdir -p "$WESTON_RUNTIME_DIR/xdg-desktop-portal"
cat > "$WESTON_RUNTIME_DIR/xdg-desktop-portal/portals.conf" <<'EOF'
[preferred]
default=gtk
EOF

XDG_RUNTIME_DIR="$WESTON_RUNTIME_DIR" \
dbus-run-session -- bash -c '
  export XDG_RUNTIME_DIR='"$WESTON_RUNTIME_DIR"'
  export XDG_SESSION_TYPE=wayland
  export XDG_CURRENT_DESKTOP=weston

  # Point xdg-desktop-portal at our custom portals.conf so it uses the
  # GTK backend instead of guessing and falling back with warnings.
  export GTK_USE_PORTAL=1
  export XDG_DESKTOP_PORTAL_DIR='"$WESTON_RUNTIME_DIR"'/xdg-desktop-portal

  # Accessibility environment
  export NO_AT_BRIDGE=0
  export ACCESSIBILITY_ENABLED=1
  export GTK_A11Y=atspi
  export QT_ACCESSIBILITY=1
  export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
  export GDK_BACKEND=wayland

  export LANG=de_DE.UTF-8
  export LC_ALL=de_DE.UTF-8

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
    DBUS_SESSION_BUS_ADDRESS="$AT_SPI_ADDR" /usr/lib/at-spi2-registryd &
    sleep 0.2
    echo "AT-SPI registryd started"
  else
    echo "WARNING: AT-SPI bus not available -- accessibility will not work" >&2
  fi

  # Start xdg-desktop-portal with the GTK backend so applications in
  # this session have a working portal (file chooser, screenshots, etc.)
  if command -v xdg-desktop-portal >/dev/null 2>&1; then
    /usr/libexec/xdg-desktop-portal-gtk &
    xdg-desktop-portal &
    echo "xdg-desktop-portal started with GTK backend"
  else
    echo "WARNING: xdg-desktop-portal not found — portals unavailable" >&2
  fi

  # Launch Weston as the compositor for this session
  exec weston \
    --backend='"$WESTON_BACKEND"'-backend.so \
    --socket='"$WESTON_SOCKET"' \
    --config='"$WESTON_INI"' \
    --width=1920 --height=1080
'
