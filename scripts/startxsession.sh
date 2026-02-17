#!/bin/bash


DISPLAY_NUM=100

Xephyr :$DISPLAY_NUM -ac -screen 1920x1080 -noreset -sw-cursor -dpi 192 &
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
  export ACCESSIBILITY_ENABLED=1
  export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
  export LANG=de_DE.UTF-8
  export LC_ALL=de_DE.UTF-8

  # optional explizit:
  /usr/lib/at-spi-bus-launcher &

  setxkbmap de -variant e2

  #exec openbox
  exec startplasma-x11
"

kill $XEPHYR_PID
