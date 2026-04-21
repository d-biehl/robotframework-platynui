#!/bin/bash
# linux-a11y-restore.sh — Restore AT-SPI ScreenReaderEnabled to its prior value.
#
# Pairs with linux-a11y-enable.sh. Reads the saved previous state from
# $XDG_RUNTIME_DIR/platynui-a11y-prev-state (or /tmp fallback) and writes
# it back to org.a11y.Status.ScreenReaderEnabled, then removes the state
# file so the next enable cycle re-saves freshly.
#
# Usage: invoked from RF Suite Teardown / pytest session-finalizer.
#
# Exit codes:
#   0 — restored successfully or no-op (non-Linux / no state file).
#   1 — gdbus missing.
#   2 — bus unreachable or write failed.
#
# Idempotent: safe to call without a prior enable; logs and exits 0.

set -u

if [ "$(uname -s)" != "Linux" ]; then
    echo "linux-a11y-restore: not Linux ($(uname -s)), no-op" >&2
    exit 0
fi

if ! command -v gdbus >/dev/null 2>&1; then
    echo "linux-a11y-restore: ERROR: gdbus not found" >&2
    exit 1
fi

_STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}"
_STATE_FILE="$_STATE_DIR/platynui-a11y-prev-state"

if [ ! -f "$_STATE_FILE" ]; then
    echo "linux-a11y-restore: no state file at $_STATE_FILE, nothing to restore" >&2
    exit 0
fi

_PREV=$(cat "$_STATE_FILE" 2>/dev/null || echo "")
case "$_PREV" in
    "true"|"false") ;;
    *)
        echo "linux-a11y-restore: WARNING: invalid saved state ('$_PREV'), defaulting to 'false'" >&2
        _PREV="false"
        ;;
esac

if ! gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
        --method=org.a11y.Bus.GetAddress >/dev/null 2>&1; then
    echo "linux-a11y-restore: ERROR: org.a11y.Bus not reachable" >&2
    exit 2
fi

if gdbus call --session --dest=org.a11y.Bus --object-path=/org/a11y/bus \
        --method=org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<$_PREV>" >/dev/null 2>&1; then
    echo "linux-a11y-restore: ScreenReaderEnabled restored to $_PREV" >&2
    rm -f "$_STATE_FILE"
    exit 0
else
    echo "linux-a11y-restore: ERROR: failed to set ScreenReaderEnabled to $_PREV" >&2
    exit 2
fi
