#!/bin/bash
set -u

# Ensure wayvnc uses the same keyboard layout as the compositor so that
# VNC clients send correctly-mapped keycodes.
export XKB_DEFAULT_LAYOUT="${XKB_DEFAULT_LAYOUT:-de}"

wayvnc -g -k de &

alacritty
