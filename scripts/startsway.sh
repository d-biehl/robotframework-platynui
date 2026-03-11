#!/bin/bash
set -u

# Start a fully isolated Sway session with its own D-Bus, AT-SPI
# accessibility bus, and XDG_RUNTIME_DIR.  Mirrors the structure of
# startcompositor.sh but uses Sway (wlroots) as the compositor.
#
# Primary use-case: testing the virtual-input backend (zwlr-virtual-pointer-v1
# + zwlr-virtual-keyboard-v1) and other wlroots-specific protocols against a
# real wlroots compositor.
#
# Usage:
#   scripts/startsway.sh [--backend winit|headless] [--resolution WxH] [--xwayland] [-- session-script args...]
#
# If no session script is given after `--`, an interactive sway session is
# started (no auto-exit).
#
# Environment variables:
#   SWAY_BACKEND           Override backend (default: auto-detect)
#   SWAY_LOG_LEVEL         Log level (default: error; use debug for verbose)
#   SWAY_EXTRA_ARGS        Additional arguments for sway

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
BACKEND="${SWAY_BACKEND:-}"
RESOLUTION="1920x1080"
XWAYLAND=1
SESSION_CMD=()
SWAY_EXTRA_ARGS_CLI=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --)
      shift
      SESSION_CMD=("$@")
      break
      ;;
    --backend)
      BACKEND="$2"
      shift 2
      ;;
    --backend=*)
      BACKEND="${1#--backend=}"
      shift
      ;;
    --resolution)
      RESOLUTION="$2"
      shift 2
      ;;
    --resolution=*)
      RESOLUTION="${1#--resolution=}"
      shift
      ;;
    --xwayland)
      XWAYLAND=1
      shift
      ;;
    --no-xwayland)
      XWAYLAND=0
      shift
      ;;
    *)
      SWAY_EXTRA_ARGS_CLI+=("$1")
      shift
      ;;
  esac
done

# ---------------------------------------------------------------------------
# Backend auto-detection
# ---------------------------------------------------------------------------
if [[ -z "$BACKEND" ]]; then
  if [[ -n "${WAYLAND_DISPLAY:-}" ]] || [[ -n "${DISPLAY:-}" ]]; then
    BACKEND="wayland"
    echo "Display detected — using wayland backend (nested)" >&2
  else
    BACKEND="headless"
    echo "No display detected — using headless backend" >&2
  fi
fi

# ---------------------------------------------------------------------------
# Isolated XDG_RUNTIME_DIR
# ---------------------------------------------------------------------------
SESSION_RUNTIME_DIR=$(mktemp -d "/run/user/$(id -u)/sway-session-XXXXXX")

cleanup() {
  if [[ -d "$SESSION_RUNTIME_DIR" ]]; then
    for mnt in "$SESSION_RUNTIME_DIR"/*/; do
      mountpoint -q "$mnt" 2>/dev/null && fusermount -u "$mnt" 2>/dev/null
    done
    rm -rf "$SESSION_RUNTIME_DIR"
  fi
}
trap cleanup EXIT INT TERM

# Under WSL, XWayland does not work reliably.
if grep -qi microsoft /proc/version 2>/dev/null; then
  echo "WSL detected — XWayland will be disabled" >&2
  XWAYLAND=0
fi

# ---------------------------------------------------------------------------
# Symlink host Wayland socket into the isolated runtime dir (for nesting)
# ---------------------------------------------------------------------------
if [[ "$BACKEND" == "wayland" ]] && [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
  PARENT_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
  PARENT_WAYLAND_SOCKET="$PARENT_RUNTIME_DIR/$WAYLAND_DISPLAY"
  if [[ -e "$PARENT_WAYLAND_SOCKET" ]]; then
    ln -sf "$PARENT_WAYLAND_SOCKET" "$SESSION_RUNTIME_DIR/$WAYLAND_DISPLAY"
    [[ -e "${PARENT_WAYLAND_SOCKET}.lock" ]] && \
      ln -sf "${PARENT_WAYLAND_SOCKET}.lock" "$SESSION_RUNTIME_DIR/${WAYLAND_DISPLAY}.lock"
    echo "Symlinked parent Wayland socket into session runtime dir" >&2
  else
    echo "WARNING: Parent Wayland socket not found at $PARENT_WAYLAND_SOCKET" >&2
    echo "         Falling back to headless backend" >&2
    BACKEND="headless"
  fi
fi

# Map our backend names to WLR_BACKENDS values
case "$BACKEND" in
  winit)    WLR_BACKEND_VALUE="wayland" ;;  # winit maps to wayland nesting
  wayland)  WLR_BACKEND_VALUE="wayland" ;;
  headless) WLR_BACKEND_VALUE="headless" ;;
  *)        WLR_BACKEND_VALUE="$BACKEND" ;;
esac

LOG_LEVEL="${SWAY_LOG_LEVEL:-error}"

echo "=== Sway Session ===" >&2
echo "Backend:          $BACKEND (WLR_BACKENDS=$WLR_BACKEND_VALUE)" >&2
echo "XWayland:         $XWAYLAND" >&2
echo "Runtime dir:      $SESSION_RUNTIME_DIR" >&2
if [[ ${#SESSION_CMD[@]} -gt 0 ]]; then
  echo "Session command:  ${SESSION_CMD[*]}" >&2
else
  echo "Session command:  (interactive — no auto-exit)" >&2
fi
if [[ -n "$RESOLUTION" ]]; then
  echo "Resolution:       $RESOLUTION" >&2
fi
echo "Log level:        $LOG_LEVEL" >&2
echo "=====================" >&2

# ---------------------------------------------------------------------------
# Generate Sway config — based on /etc/sway/config defaults
# ---------------------------------------------------------------------------
SWAY_CONFIG="$SESSION_RUNTIME_DIR/sway-config"

cat > "$SWAY_CONFIG" <<'SWAY_CONFIG_EOF'
# PlatynUI Sway config — generated by scripts/startsway.sh

### Variables
set $mod Mod4
set $left h
set $down j
set $up k
set $right l
set $term foot
set $menu wmenu-run

# Keyboard layout
input type:keyboard {
    xkb_layout de
}

### Output configuration
#
# Headless outputs are named HEADLESS-1, HEADLESS-2, ...
# Nested wayland outputs are named WL-1, WL-2, ...
output * bg /usr/share/backgrounds/sway/Sway_Wallpaper_Blue_1920x1080.png fill

### Key bindings — Basics
    bindsym $mod+Return exec $term
    bindsym $mod+Shift+q kill
    bindsym $mod+d exec $menu
    floating_modifier $mod normal
    bindsym $mod+Shift+c reload
    bindsym $mod+Shift+e exec swaynag -t warning -m 'Exit sway?' -B 'Yes, exit sway' 'swaymsg exit'

### Moving around
    bindsym $mod+$left focus left
    bindsym $mod+$down focus down
    bindsym $mod+$up focus up
    bindsym $mod+$right focus right
    bindsym $mod+Left focus left
    bindsym $mod+Down focus down
    bindsym $mod+Up focus up
    bindsym $mod+Right focus right

    bindsym $mod+Shift+$left move left
    bindsym $mod+Shift+$down move down
    bindsym $mod+Shift+$up move up
    bindsym $mod+Shift+$right move right
    bindsym $mod+Shift+Left move left
    bindsym $mod+Shift+Down move down
    bindsym $mod+Shift+Up move up
    bindsym $mod+Shift+Right move right

### Workspaces
    bindsym $mod+1 workspace number 1
    bindsym $mod+2 workspace number 2
    bindsym $mod+3 workspace number 3
    bindsym $mod+4 workspace number 4
    bindsym $mod+5 workspace number 5
    bindsym $mod+6 workspace number 6
    bindsym $mod+7 workspace number 7
    bindsym $mod+8 workspace number 8
    bindsym $mod+9 workspace number 9
    bindsym $mod+0 workspace number 10
    bindsym $mod+Shift+1 move container to workspace number 1
    bindsym $mod+Shift+2 move container to workspace number 2
    bindsym $mod+Shift+3 move container to workspace number 3
    bindsym $mod+Shift+4 move container to workspace number 4
    bindsym $mod+Shift+5 move container to workspace number 5
    bindsym $mod+Shift+6 move container to workspace number 6
    bindsym $mod+Shift+7 move container to workspace number 7
    bindsym $mod+Shift+8 move container to workspace number 8
    bindsym $mod+Shift+9 move container to workspace number 9
    bindsym $mod+Shift+0 move container to workspace number 10

### Layout
    bindsym $mod+b splith
    bindsym $mod+v splitv
    bindsym $mod+s layout stacking
    bindsym $mod+w layout tabbed
    bindsym $mod+e layout toggle split
    bindsym $mod+f fullscreen
    bindsym $mod+Shift+space floating toggle
    bindsym $mod+space focus mode_toggle
    bindsym $mod+a focus parent

### Scratchpad
    bindsym $mod+Shift+minus move scratchpad
    bindsym $mod+minus scratchpad show

### Resize mode
mode "resize" {
    bindsym $left resize shrink width 10px
    bindsym $down resize grow height 10px
    bindsym $up resize shrink height 10px
    bindsym $right resize grow width 10px
    bindsym Left resize shrink width 10px
    bindsym Down resize grow height 10px
    bindsym Up resize shrink height 10px
    bindsym Right resize grow width 10px
    bindsym Return mode "default"
    bindsym Escape mode "default"
}
bindsym $mod+r mode "resize"

### Utilities
    bindsym --locked XF86AudioMute exec pactl set-sink-mute \@DEFAULT_SINK@ toggle
    bindsym --locked XF86AudioLowerVolume exec pactl set-sink-volume \@DEFAULT_SINK@ -5%
    bindsym --locked XF86AudioRaiseVolume exec pactl set-sink-volume \@DEFAULT_SINK@ +5%
    bindsym --locked XF86AudioMicMute exec pactl set-source-mute \@DEFAULT_SOURCE@ toggle
    bindsym --locked XF86MonBrightnessDown exec brightnessctl set 5%-
    bindsym --locked XF86MonBrightnessUp exec brightnessctl set 5%+
    bindsym Print exec grim

### Status bar
bar {
    position top
    status_command while date +'%Y-%m-%d %X'; do sleep 1; done
    colors {
        statusline #ffffff
        background #323232
        inactive_workspace #32323200 #32323200 #5c5c5c
    }
}

SWAY_CONFIG_EOF

# Output resolution (--custom allows arbitrary sizes not in EDID)
# Only applied in headless mode — in nested mode the parent compositor controls
# the window size and forcing a mode prevents interactive resizing.
if [[ -n "$RESOLUTION" ]] && [[ "$BACKEND" == "headless" ]]; then
  echo "output * mode --custom $RESOLUTION" >> "$SWAY_CONFIG"
fi

# XWayland
if [[ "$XWAYLAND" -eq 0 ]]; then
  echo "xwayland disable" >> "$SWAY_CONFIG"
fi

# If a session command is provided, exec it and exit sway when it finishes
if [[ ${#SESSION_CMD[@]} -gt 0 ]]; then
  SERIALIZED_SESSION="$(printf '%q ' "${SESSION_CMD[@]}")"
  cat >> "$SWAY_CONFIG" <<EXEC_EOF

# Auto-launch session command and exit when done
exec bash -c ${SERIALIZED_SESSION@Q}'; swaymsg exit 2>/dev/null || true'
EXEC_EOF
fi

# ---------------------------------------------------------------------------
# Compute sway flags before generating the inner script
# ---------------------------------------------------------------------------
SWAY_DEBUG_FLAG=""
if [[ "$LOG_LEVEL" == "debug" ]] || [[ "$LOG_LEVEL" == "trace" ]]; then
  SWAY_DEBUG_FLAG="-d"
fi

# Build the full sway argument list
SWAY_ARGS=(-c "$SWAY_CONFIG")
[[ -n "$SWAY_DEBUG_FLAG" ]] && SWAY_ARGS+=("$SWAY_DEBUG_FLAG")

# Pass any extra args from SWAY_EXTRA_ARGS env or CLI
if [[ -n "${SWAY_EXTRA_ARGS:-}" ]]; then
  # shellcheck disable=SC2206
  SWAY_ARGS+=($SWAY_EXTRA_ARGS)
fi
SWAY_ARGS+=("${SWAY_EXTRA_ARGS_CLI[@]}")

SERIALIZED_SWAY_ARGS="$(printf '%q ' "${SWAY_ARGS[@]}")"

# ---------------------------------------------------------------------------
# Write the inner session bootstrap script
# ---------------------------------------------------------------------------
INNER_SCRIPT="$SESSION_RUNTIME_DIR/inner-session.sh"

cat > "$INNER_SCRIPT" <<INNER_EOF
#!/bin/bash
set -u

export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=sway

# Accessibility environment
export NO_AT_BRIDGE=0
export ACCESSIBILITY_ENABLED=1
export GTK_A11Y=atspi
export QT_ACCESSIBILITY=1
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export GDK_BACKEND=wayland
export QT_QPA_PLATFORM=wayland

export LANG=de_DE.UTF-8
export LC_ALL=de_DE.UTF-8

# wlroots backend selection
export WLR_BACKENDS=${WLR_BACKEND_VALUE@Q}

# Suppress "no input devices" warning in headless mode
export WLR_LIBINPUT_NO_DEVICES=1

# Use software cursor (no hardware cursor plane in headless/nested)
export WLR_NO_HARDWARE_CURSORS=1

echo "Session XDG_RUNTIME_DIR=\$XDG_RUNTIME_DIR" >&2
echo "Session DBUS_SESSION_BUS_ADDRESS=\$DBUS_SESSION_BUS_ADDRESS" >&2

exec sway $SERIALIZED_SWAY_ARGS
INNER_EOF
chmod +x "$INNER_SCRIPT"

# ---------------------------------------------------------------------------
# Launch inside an isolated D-Bus session
# ---------------------------------------------------------------------------
unset DBUS_SESSION_BUS_ADDRESS
unset AT_SPI_BUS_ADDRESS
unset QT_IM_MODULE
unset QT_IM_MODULES
unset SWAYSOCK
unset I3SOCK
unset PLATYNUI_CONTROL_SOCKET
unset LIBEI_SOCKET

XDG_RUNTIME_DIR="$SESSION_RUNTIME_DIR" \
  dbus-run-session -- "$INNER_SCRIPT"
