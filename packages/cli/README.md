# Robot Framework PlatynUI – CLI

Command‑line interface for the PlatynUI toolkit. This tool lets you inspect the desktop, query the accessibility tree via XPath‑like expressions, highlight elements, take screenshots, and drive input (focus, window control, pointer, keyboard).

Status: early alpha. APIs and output may change.

> [!WARNING]
> Preview release on PyPI. This package is experimental and the CLI surface may change without notice. Use for evaluation, not production.

## Install

Install the preview (pre-release) package from PyPI into your environment. After installation, the command `platynui-cli` is available on your PATH.

Examples:

```pwsh
# In a virtual environment (uv)
uv pip install --pre robotframework-platynui-cli

# Or with pip
pip install --pre robotframework-platynui-cli

# Or install as a user-level tool (uv tool)
uv tool install --prerelease allow robotframework-platynui-cli
```

Supported platforms: Windows, macOS, and Linux. The binary includes the platform providers for your OS.
Note: Because this is a pre-release, some installers require an explicit flag (e.g., `--pre`) to select preview versions.

## Quick start

```pwsh
platynui-cli list-providers
platynui-cli info --format json
platynui-cli query "//control:Button[@Name='OK']"
platynui-cli highlight "//control:Button[@Name='OK']" --duration-ms 1200
platynui-cli screenshot .\screen.png --rect 100,100,800,600
```

Most commands support `--format text|json` (default: text) where it makes sense.

## Commands

- list-providers
	- List all known UI tree providers; marks which are currently active.
	- Options: `--format text|json`

- info
	- Show desktop and monitor metadata (bounds, OS, displays).
	- Options: `--format text|json`

- query XPATH
	- Evaluate an XPath expression against the UI tree. Prints nodes, attributes, or values.
	- Options: `--format text|json`
	- Example: `platynui-cli query "//control:Button[@Name='OK']"`

- watch
	- Stream provider events (NodeAdded/Updated/Removed, TreeInvalidated).
	- Options: `--format text|json`, `--expression XPATH` (optional additional query run per event), `--limit N`

- highlight [XPATH] | --rect X,Y,WIDTH,HEIGHT
	- Draw a temporary highlight overlay for matched elements or a given rectangle.
	- Options: `--duration-ms MS` (default 1500), `--clear` (clear existing highlights)
	- Example: `platynui-cli highlight "//control:Button[@Name='OK']"`

- screenshot [FILE]
	- Capture a PNG screenshot. If FILE is omitted, a timestamped name is created in the current directory.
	- Options: `--rect X,Y,WIDTH,HEIGHT`

- focus XPATH
	- Set input focus to the first node matched by the expression. Reports if the Focusable pattern is missing.

- window [XPATH] --list | --activate | --minimize | --maximize | --restore | --close | --move X Y | --resize W H
	- List and/or control windows matched by XPATH. Without XPATH, defaults to `//control:Window`.
	- Example: `platynui-cli window --list`
	- Example: `platynui-cli window "//control:Window[@Name='Operations Console']" --activate --move 200 220 --resize 800 600`

- pointer …
	- Drive pointer input. All subcommands accept advanced overrides (see below).
	- Subcommands:
		- `move [XPATH] [--point x,y]` – Move to element (uses @ActivationPoint or center of @Bounds) or to a point.
		- `click [XPATH] [--point x,y] [--button left|right|middle|<code>] [--no-move]`
		- `multi-click [XPATH] [--point x,y] [--button …] --count N [--no-move]`
		- `press [XPATH] [--point x,y] [--button …] [--no-move]`
		- `release [XPATH] [--button …] [--no-move]`
		- `scroll <dx,dy> [--expr XPATH] [--no-move]`
		- `drag --from x,y --to x,y [--from-expr XPATH] [--to-expr XPATH] [--button …]`
		- `position` – Print current pointer position.
	- Useful overrides (apply to all pointer subcommands via flags):
		- `--origin desktop|bounds|absolute` with `--bounds x,y,w,h` or `--anchor x,y`
		- Motion/profile: `--motion direct|linear|bezier|overshoot|jitter`, `--move-duration MS`, `--speed-factor F`,
			`--acceleration constant|ease-in|ease-out|smooth-step`
		- Timing: `--after-move MS`, `--after-input MS`, `--press-release MS`, `--after-click MS`,
			`--before-next MS`, `--multi-click MS`, `--ensure-threshold PX`, `--ensure-timeout MS`,
			`--scroll-delay MS`, `--scroll-step dx,dy`, `--move-time-per-pixel MS`

- keyboard …
	- Send keyboard input.
	- Subcommands:
		- `type "SEQUENCE"` – Types text and chords. Example: `<Ctrl+A>Hallo`.
		- `press "<Chord>"` – Press and hold keys. Example: `<Shift+Ctrl+S>`.
		- `release "<Chord>"` – Release keys from a previous press.
	- Timing overrides: `--delay-ms`, `--press-delay`, `--release-delay`, `--between-keys-delay`,
		`--chord-press-delay`, `--chord-release-delay`, `--after-sequence-delay`, `--after-text-delay`

## XPath quick notes

- Default namespace is `control` (e.g., `//control:Button`).
- Attributes use PascalCase, for example `@Name`, `@Bounds`, `@ActivationPoint`.
- `query` outputs a colored text view by default; use `--format json` for machine‑readable output.

## Troubleshooting

- If nothing shows up in `list-providers`, ensure the build includes your OS provider (it is linked by default when building the CLI in this workspace).
- On Linux, accessibility needs to be enabled and AT‑SPI running. On macOS, grant accessibility permissions to your shell/terminal. On Windows, UIA should be available by default.

## License

Apache-2.0. See the repository’s LICENSE file.
