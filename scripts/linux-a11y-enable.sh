#!/bin/bash
# linux-a11y-enable.sh — Force-enable AT-SPI accessibility for AccessKit clients.
#
# Background:
#   accesskit_unix (used by eframe/egui via accesskit_winit) only registers
#   its D-Bus adapter on the AT-SPI bus while org.a11y.Status.IsEnabled is
#   true (accesskit_unix >= 0.22 watches IsEnabled; older releases watched
#   ScreenReaderEnabled, which at-spi2-core 2.62 removed). Without this, apps
#   like apps/test-app-egui are invisible to the AT-SPI provider, even though
#   AccessKit itself is initialized.
#
#   at-spi2-core >= 2.62 also switches IsEnabled on by itself while an
#   assistive technology (the AT-SPI provider included) has event listeners
#   registered, and off again once the last one is gone; older launchers keep
#   whatever was set. This script flips the flag to true so RF acceptance
#   suites and ad-hoc smoke tests can observe the tree on either. It saves the
#   previous value to a state file so linux-a11y-restore.sh can put it back.
#
# Usage:
#   scripts/linux-a11y-enable.sh
#   # ...run tests...
#   scripts/linux-a11y-restore.sh
#
# Exit codes:
#   0 — Linux: state set to true (or already true) and saved; non-Linux: no-op.
#   1 — Required tooling (gdbus) missing.
#   2 — org.a11y.Bus not reachable on the session bus.
#
# Idempotent: safe to call multiple times; only the FIRST call writes the
# state file, subsequent calls leave the saved previous value untouched.

set -u

# --- Platform guard ----------------------------------------------------------
if [ "$(uname -s)" != "Linux" ]; then
    echo "linux-a11y-enable: not Linux ($(uname -s)), no-op" >&2
    exit 0
fi

# --- Tool check --------------------------------------------------------------
if ! command -v gdbus >/dev/null 2>&1; then
    echo "linux-a11y-enable: ERROR: gdbus not found (install glib2/gio tools)" >&2
    exit 1
fi

# --- State file location -----------------------------------------------------
_STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}"
_STATE_FILE="$_STATE_DIR/platynui-a11y-prev-state"

# --- Bus reachability --------------------------------------------------------
if ! gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
        --method=org.a11y.Bus.GetAddress >/dev/null 2>&1; then
    echo "linux-a11y-enable: ERROR: org.a11y.Bus not reachable on session bus" >&2
    echo "                   (is at-spi2-core installed and the user session active?)" >&2
    exit 2
fi

# --- Read current value ------------------------------------------------------
# gdbus output for a boolean property: "(<true>,)" or "(<false>,)"
_CURRENT_RAW=$(gdbus call --session --dest=org.a11y.Bus \
    --object-path=/org/a11y/bus \
    --method=org.freedesktop.DBus.Properties.Get \
    org.a11y.Status IsEnabled 2>/dev/null || echo "")

case "$_CURRENT_RAW" in
    *"true"*)  _CURRENT="true"  ;;
    *"false"*) _CURRENT="false" ;;
    *)
        echo "linux-a11y-enable: WARNING: could not parse current state ($_CURRENT_RAW)" >&2
        echo "                   assuming 'false' for restore purposes" >&2
        _CURRENT="false"
        ;;
esac

# --- Save previous value (only on first invocation) --------------------------
if [ ! -f "$_STATE_FILE" ]; then
    echo "$_CURRENT" > "$_STATE_FILE"
    echo "linux-a11y-enable: saved previous IsEnabled=$_CURRENT to $_STATE_FILE" >&2
else
    _SAVED=$(cat "$_STATE_FILE" 2>/dev/null || echo "?")
    echo "linux-a11y-enable: state file already present (saved=$_SAVED), not overwriting" >&2
fi

# --- Set to true (idempotent) ------------------------------------------------
if [ "$_CURRENT" = "true" ]; then
    echo "linux-a11y-enable: IsEnabled already true, nothing to do" >&2
    exit 0
fi

if gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
        --method=org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null 2>&1; then
    echo "linux-a11y-enable: IsEnabled set to true" >&2
    exit 0
else
    echo "linux-a11y-enable: ERROR: failed to set IsEnabled" >&2
    exit 2
fi
