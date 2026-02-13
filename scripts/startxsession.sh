#!/bin/bash


DISPLAY_NUM=100

Xephyr :$DISPLAY_NUM -screen 1920x1080 -nolisten tcp -noreset -dpi 192 &
XEPHYR_PID=$!

unset DBUS_SESSION_BUS_ADDRESS
unset WAYLAND_DISPLAY
unset XAUTHORITY

sleep 1

dbus-run-session -- bash -c "
  export DISPLAY=:$DISPLAY_NUM
  export XDG_SESSION_TYPE=x11
  export XDG_CURRENT_DESKTOP=xephyr-test
  export NO_AT_BRIDGE=0

  # optional explizit:
  /usr/lib/at-spi2-core/at-spi-bus-launcher &

  exec openbox
"

kill $XEPHYR_PID
