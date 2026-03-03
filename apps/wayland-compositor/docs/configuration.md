# Configuration

The compositor can be configured via a TOML file. All settings are optional —
missing values use built-in defaults. CLI flags always override config file values.

## File Discovery

The config file is discovered in this order (first match wins):

1. `--config <path>` CLI flag (explicit path)
2. `$XDG_CONFIG_HOME/platynui/compositor.toml`
3. `~/.config/platynui/compositor.toml` (if `XDG_CONFIG_HOME` is unset)
4. No file → built-in defaults (the compositor runs fine without any config file)

## Complete Example

```toml
# ~/.config/platynui/compositor.toml

# ── Font ─────────────────────────────────────────────────────────────
# Font for compositor-rendered UI (title bars, panel).
# Resolved via fontconfig at runtime; falls back to egui's built-in font.
[font]
family = "Noto Sans"     # Font family name (default: "Noto Sans")
size = 13.0              # Font size in logical pixels (default: 13.0)

# ── Theme ────────────────────────────────────────────────────────────
# Colors for window decorations. CSS-style hex strings: #rrggbb or #rrggbbaa.
# Invalid values silently fall back to the built-in defaults.
[theme]
titlebar-background         = "#33333f"   # Inactive window title bar
titlebar-background-focused = "#404d73"   # Focused window title bar
titlebar-text               = "#ffffff"   # Title bar text
button-close                = "#d94040"   # Close (✕) button
button-maximize             = "#40bf59"   # Maximize (□) button
button-minimize             = "#e6bf33"   # Minimize (─) button
active-border               = "#7380b3"   # Focused window border
inactive-border             = "#595966"   # Unfocused window border

# ── Keyboard ─────────────────────────────────────────────────────────
# XKB keyboard configuration. Equivalent to XKB_DEFAULT_* env vars and
# --keyboard-* CLI flags.
[keyboard]
model   = "pc105"                        # XKB model (e.g. "pc105")
rules   = "evdev"                        # XKB rules file
options = "grp:alt_shift_toggle,compose:ralt"  # XKB options, comma-separated

# Multiple layouts as an array of tables. Each entry has a name and an
# optional variant. Layouts are joined with commas for XKB (de,us → "de,us").
[[keyboard.layout]]
name    = "de"
variant = "nodeadkeys"

[[keyboard.layout]]
name    = "us"
# variant omitted → default variant

# ── Outputs ──────────────────────────────────────────────────────────
# Virtual monitor definitions. Overrides --outputs/--width/--height CLI flags
# when present. Output entries are applied in order.
[[output]]
width  = 1920            # Width in pixels (default: 1920)
height = 1080            # Height in pixels (default: 1080)
x      = 0               # X position in the combined output space (default: 0)
y      = 0               # Y position in the combined output space (default: 0)
scale  = 1.0             # Scale factor, e.g. 1.0, 1.5, 2.0 (default: 1.0)

[[output]]
width  = 2560
height = 1440
x      = 1920
y      = 0
scale  = 1.5
```

## Section Reference

### `[font]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `family` | string | `"Noto Sans"` | Font family name. Resolved via fontconfig; falls back to egui built-in. |
| `size` | float | `13.0` | Font size in logical pixels. |

### `[theme]`

All theme values are CSS-style hex color strings: `#rrggbb` (opaque) or `#rrggbbaa` (with alpha).
Invalid values silently fall back to the built-in defaults listed below.

| Key | Default | Description |
|-----|---------|-------------|
| `titlebar-background` | `#33333f` | Title bar background for inactive windows |
| `titlebar-background-focused` | `#404d73` | Title bar background for the focused window |
| `titlebar-text` | `#ffffff` | Title bar text color |
| `button-close` | `#d94040` | Close button color |
| `button-maximize` | `#40bf59` | Maximize button color |
| `button-minimize` | `#e6bf33` | Minimize button color |
| `active-border` | `#7380b3` | Border color for the focused window |
| `inactive-border` | `#595966` | Border color for unfocused windows |

### `[keyboard]`

| Key | Type | Default | Equivalent CLI / Env |
|-----|------|---------|---------------------|
| `model` | string | — | `--keyboard-model` / `XKB_DEFAULT_MODEL` |
| `rules` | string | — | `--keyboard-rules` / `XKB_DEFAULT_RULES` |
| `options` | string | — | `--keyboard-options` / `XKB_DEFAULT_OPTIONS` |

### `[[keyboard.layout]]`

Array of tables. Each entry defines one XKB layout. Multiple entries are joined
with commas (e.g. `de,us`) and passed to XKB as a combined layout string.

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | XKB layout name (e.g. `de`, `us`, `fr`) |
| `variant` | string | no | XKB variant (e.g. `nodeadkeys`, `neo`). Omit for default. |

**Priority chain:** CLI flag (`--keyboard-layout`) > config file (`[[keyboard.layout]]`) > environment variable (`XKB_DEFAULT_LAYOUT`) > XKB compiled-in default.

### `[[output]]`

Array of tables. Each entry defines one virtual monitor. When `[[output]]` entries
are present, they override `--outputs`, `--width`, and `--height` CLI flags.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `width` | integer | `1920` | Width in pixels |
| `height` | integer | `1080` | Height in pixels |
| `x` | integer | `0` | X position in combined output space |
| `y` | integer | `0` | Y position in combined output space |
| `scale` | float | `1.0` | Output scale factor (e.g. `1.0`, `1.5`, `2.0`) |

## Minimal Configurations

Only specify what you need — everything has sensible defaults:

```toml
# Just change the font
[font]
family = "DejaVu Sans"
```

```toml
# German keyboard with nodeadkeys
[[keyboard.layout]]
name = "de"
variant = "nodeadkeys"
```

```toml
# Dark red theme for close buttons
[theme]
button-close = "#cc3333"
```
