#!/bin/bash
# setup-atspi.sh — Start the AT-SPI accessibility bus in an isolated session.
#
# Source this script from your session script (runs inside dbus-run-session)
# BEFORE launching any applications that need accessibility.
#
# Usage (from a session script):
#   source "$(dirname "$0")/setup-atspi.sh"
#
# Requires: dbus-send, at-spi-bus-launcher, at-spi2-registryd
#
# Background:
#   Our isolated session has no systemd --user, and dbus-broker (used by many
#   distros as session bus) relies on systemd for D-Bus service activation.
#   Therefore we must start at-spi-bus-launcher and at-spi2-registryd
#   explicitly and ensure registryd has claimed its bus name BEFORE any
#   client connects.

# Mask org.freedesktop.systemd1 on the session bus to prevent dbus-broker
# from repeatedly trying (and failing) to activate it.
_ATSPI_SERVICES_DIR="$XDG_RUNTIME_DIR/at-spi-services/dbus-1/services"
mkdir -p "$_ATSPI_SERVICES_DIR"
cat > "$_ATSPI_SERVICES_DIR/org.freedesktop.systemd1.service" <<'_MASK_EOF'
# Intentionally empty — masks the system service in this isolated session.
_MASK_EOF

# Prepend our override directory to XDG_DATA_DIRS.
export XDG_DATA_DIRS="$XDG_RUNTIME_DIR/at-spi-services:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"

# 1) Start AT-SPI bus launcher.
/usr/lib/at-spi-bus-launcher --launch-immediately --a11y=1 &
_ATSPI_LAUNCHER_PID=$!

# 2) Wait until org.a11y.Bus is available on the session bus.
_ATSPI_READY=0
for _i in $(seq 1 50); do
  if dbus-send --session --dest=org.a11y.Bus --print-reply \
       /org/a11y/bus org.a11y.Bus.GetAddress >/dev/null 2>&1; then
    echo "AT-SPI bus launcher ready after $((_i * 100))ms" >&2
    _ATSPI_READY=1
    break
  fi
  sleep 0.1
done

if [ "$_ATSPI_READY" -eq 0 ]; then
  echo "WARNING: AT-SPI bus launcher did not become ready within 5s" >&2
fi

# 3) Get the AT-SPI accessibility bus address.
AT_SPI_ADDR=$(dbus-send --session --dest=org.a11y.Bus --print-reply \
  /org/a11y/bus org.a11y.Bus.GetAddress 2>/dev/null \
  | grep string | head -1 | sed 's/.*"\(.*\)"/\1/')

if [ -n "$AT_SPI_ADDR" ]; then
  echo "AT-SPI accessibility bus at: $AT_SPI_ADDR" >&2
  export AT_SPI_BUS_ADDRESS="$AT_SPI_ADDR"

  # 4) Manually start registryd on the AT-SPI bus.
  DBUS_SESSION_BUS_ADDRESS="$AT_SPI_ADDR" /usr/lib/at-spi2-registryd &
  _REGISTRYD_PID=$!

  # 5) Wait until registryd has claimed org.a11y.atspi.Registry.
  _REGISTRY_READY=0
  for _i in $(seq 1 50); do
    if DBUS_SESSION_BUS_ADDRESS="$AT_SPI_ADDR" \
       dbus-send --session --dest=org.a11y.atspi.Registry --print-reply \
         /org/a11y/atspi/accessible/root org.freedesktop.DBus.Peer.Ping \
         >/dev/null 2>&1; then
      echo "AT-SPI registryd ready after $((_i * 100))ms" >&2
      _REGISTRY_READY=1
      break
    fi
    sleep 0.1
  done

  if [ "$_REGISTRY_READY" -eq 0 ]; then
    echo "WARNING: AT-SPI registryd did not become ready within 5s" >&2
    if ! kill -0 "$_REGISTRYD_PID" 2>/dev/null; then
      echo "         registryd (PID $_REGISTRYD_PID) is no longer running!" >&2
    fi
  fi
else
  echo "WARNING: AT-SPI bus not available — accessibility will not work" >&2
fi

# Clean up local variables (keep AT_SPI_BUS_ADDRESS exported).
unset _ATSPI_SERVICES_DIR _ATSPI_LAUNCHER_PID _ATSPI_READY _REGISTRYD_PID _REGISTRY_READY _i
