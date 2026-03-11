#!/bin/bash
set -u

# Default session script for the PlatynUI compositor.  Launched as the
# compositor's child process (via --exit-with-child).
#
# Sets up the AT-SPI accessibility bus (required for PlatynUI's AT-SPI
# provider), then starts wayvnc and alacritty.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Source AT-SPI setup (shared helper — usable by any session script).
source "$SCRIPT_DIR/setup-atspi.sh"

# Ensure wayvnc uses the same keyboard layout as the compositor so that
# VNC clients send correctly-mapped keycodes.
export XKB_DEFAULT_LAYOUT="${XKB_DEFAULT_LAYOUT:-de}"

# wayvnc -g -k de &

alacritty
