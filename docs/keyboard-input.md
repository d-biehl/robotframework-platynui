# Keyboard Input

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

PlatynUI uses a simple, consistent syntax for keyboard input — the same across CLI, Python, and Robot Framework.

## 1. The Basics

A keyboard sequence is built from two elements:

- **Text** — typed character by character: `Hello World`
- **Special keys & shortcuts** — wrapped in angle brackets: `<Ctrl+C>`

Both can be freely combined:

```
Hello World<Return>
```

Types "Hello World" and then presses Enter.

## 2. Typing Text

Anything outside of `< >` is treated as plain text:

```
Hello World
```

Each character is typed individually — including spaces, punctuation, and special characters.

## 3. Special Keys & Shortcuts

Keys like Enter, Tab, or Escape go inside angle brackets:

```
<Return>
<Tab>
<Escape>
<F5>
```

For key combinations, join keys with `+`:

```
<Ctrl+C>          Copy
<Ctrl+V>          Paste
<Ctrl+Shift+S>    Save as
<Alt+F4>          Close window
```

PlatynUI presses all keys in order, then releases them in reverse — just like a human would.

### Multiple Shortcuts in One Block

A single block can contain multiple combinations separated by spaces:

```
<Ctrl+A Ctrl+C>
```

This is shorthand for `<Ctrl+A><Ctrl+C>` — select all, then copy.

## 4. Mixing Text and Shortcuts

```
Hello<Tab>World<Return>
```

1. Types "Hello"
2. Presses Tab
3. Types "World"
4. Presses Enter

Another example:

```
<Ctrl+A><Ctrl+C>New text<Ctrl+V>
```

1. Select all
2. Copy
3. Type "New text"
4. Paste

## 5. Holding Keys (Press / Release)

Normally every key is pressed and released immediately (**Type** mode). Sometimes you need to hold a key — that's what **Press** and **Release** are for:

```robotframework
Keyboard Press     ${None}    <Shift>
Keyboard Type      ${None}    hello         # → types "HELLO"
Keyboard Release   ${None}    <Shift>
```

## 6. Special Characters & Escapes

The characters `<`, `>`, and `\` have special meaning in the sequence syntax. To type them as literal text, prefix them with a backslash:

| Input | Result |
|-------|--------|
| `\<` | Types `<` |
| `\>` | Types `>` |
| `\\` | Types `\` |

For Unicode characters:

| Input | Result |
|-------|--------|
| `\u00E4` | ä |
| `\u00F6` | ö |
| `\u00FC` | ü |
| `\x41` | A (hex value) |

**Example:**

```
Price: 5 \< 10         →  types "Price: 5 < 10"
C:\\Users\\test        →  types "C:\Users\test"
```

> **Robot Framework note:** RF processes `\` before the string reaches PlatynUI. Double the backslash in `.robot` files:
>
> | RF source | PlatynUI receives | Result |
> |-----------|-------------------|--------|
> | `\\<` | `\<` | types `<` |
> | `\\\\` | `\\` | types `\` |
> | `\\u00E4` | `\u00E4` | types `ä` |

## 7. Symbols in Shortcuts

Some characters have special meaning inside `< >` (`+` separates keys, `<`/`>` delimit the block). Use these named aliases instead:

| Alias | Character | Example |
|-------|-----------|---------|
| `PLUS` | `+` | `<Ctrl+PLUS>` — Ctrl and Plus |
| `MINUS` | `-` | `<Ctrl+MINUS>` — Ctrl and Minus |
| `LESS` or `LT` | `<` | `<Ctrl+LESS>` |
| `GREATER` or `GT` | `>` | `<Ctrl+GREATER>` |

Other special characters like `#`, `.`, or `,` can be used directly:

```
<Ctrl+#>
<Ctrl+Shift+.>
```

## 8. Available Key Names

All key names are **case-insensitive** — `Ctrl`, `CTRL`, and `ctrl` all work.

PlatynUI aims for a single set of key names that works on every platform. The tables below list the canonical names. macOS support is planned — when it lands, the same names will work there too.

### Modifiers

| Key | Aliases | Notes |
|-----|---------|-------|
| `Ctrl` | `Control` | |
| `Shift` | | |
| `Alt` | | |
| `AltGr` | `RAlt` | Right Alt / AltGr |
| `Win` | `Windows`, `Super` | Windows key / Super |
| `Command` | `Cmd` | macOS ⌘ (planned) |
| `Option` | | macOS ⌥ (planned) |
| `Meta` | | Linux Meta key |

Left/right variants: `LCtrl`, `RCtrl`, `LShift`, `RShift`, `LAlt`, `LWin`, `RWin`

### Common Keys

| Key | Aliases |
|-----|---------|
| `Return` | `Enter` |
| `Escape` | `Esc` |
| `Tab` | |
| `Space` | |
| `Backspace` | |
| `Delete` | `Del` |
| `Insert` | `Ins` |

### Navigation

| Key | Aliases |
|-----|---------|
| `Home` | |
| `End` | |
| `PageUp` | `PgUp` |
| `PageDown` | `PgDn` |
| `Left` | `ArrowLeft` |
| `Right` | `ArrowRight` |
| `Up` | `ArrowUp` |
| `Down` | `ArrowDown` |

### Function Keys

`F1` through `F24`

### Numpad

| Key | Aliases |
|-----|---------|
| `Numpad0`–`Numpad9` | |
| `NumpadAdd` | `Add` |
| `NumpadSubtract` | `Subtract` |
| `NumpadMultiply` | `Multiply` |
| `NumpadDivide` | `Divide` |
| `NumpadDecimal` | `Decimal` |
| `NumpadEnter` | |

### System

| Key | Aliases |
|-----|---------|
| `PrintScreen` | `PrtSc` |
| `Pause` | |
| `CapsLock` | |
| `NumLock` | |
| `ScrollLock` | |

> **Tip:** Run `platynui-cli keyboard list` to see all key names available on your platform.

## 9. Quick Reference

| What you want to do | Sequence |
|----------------------|----------|
| Type text | `Hello World` |
| Press Enter | `<Return>` |
| Copy | `<Ctrl+C>` |
| Paste | `<Ctrl+V>` |
| Select all | `<Ctrl+A>` |
| Undo | `<Ctrl+Z>` |
| Save | `<Ctrl+S>` |
| Close window | `<Alt+F4>` |
| Switch window | `<Alt+Tab>` |
| Task manager | `<Ctrl+Shift+Escape>` |
| Tab then text | `<Tab>Hello` |
| Select all + copy | `<Ctrl+A Ctrl+C>` |
| Special characters | `\u00E4` (ä), `\u00F6` (ö), `\u00FC` (ü) |
| Type angle bracket | `\<` |
| Plus sign in shortcut | `<PLUS>` |

## 10. Usage

### CLI

```bash
platynui-cli keyboard type "Hello<Return>"
platynui-cli keyboard type "<Ctrl+A><Ctrl+C>"
platynui-cli keyboard press "<Shift>"
platynui-cli keyboard release "<Shift>"
platynui-cli keyboard list
```

### Python

```python
from platynui_native.runtime import Runtime

rt = Runtime()

rt.keyboard_type("Hello<Return>")
rt.keyboard_type("<Ctrl+A><Ctrl+C>")

# Hold a key
rt.keyboard_press("<Shift>")
rt.keyboard_type("hello")       # → "HELLO"
rt.keyboard_release("<Shift>")
```

### Robot Framework

```robotframework
*** Settings ***
Library    PlatynUI

*** Test Cases ***
Type Into Text Field
    Keyboard Type    ${text_field}    Hello World<Return>

Select All And Copy
    Keyboard Type    ${text_field}    <Ctrl+A><Ctrl+C>

Hold A Modifier Key
    Keyboard Press     ${None}    <Shift>
    Keyboard Type      ${None}    hello
    Keyboard Release   ${None}    <Shift>

Type Special Characters
    # RF consumes one backslash — double it so PlatynUI sees the escape
    Keyboard Type    ${None}    Price: 5 \\< 10       # → types "Price: 5 < 10"
    Keyboard Type    ${None}    C:\\\\Users\\\\test    # → types "C:\Users\test"
    Keyboard Type    ${None}    \\u00E4               # → types "ä"
```

The first parameter (`descriptor`) specifies the target element. PlatynUI automatically focuses it before sending input. Pass `${None}` to send input without focusing a specific element.

## 11. Adjusting Timing

PlatynUI inserts small pauses between key events so applications can process the input reliably. The defaults are tuned for fast test automation (~240 WPM) while keeping events spaced enough for applications to handle them correctly.

### Default Timing Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `press_delay` | 25 ms | How long a key is held down |
| `release_delay` | 5 ms | Pause after releasing a key |
| `between_keys_delay` | 20 ms | Pause between consecutive keystrokes |
| `chord_press_delay` | 12 ms | Pause between pressing keys in a combination (e.g. Ctrl+C) |
| `chord_release_delay` | 12 ms | Pause between releasing keys in a combination |
| `after_sequence_delay` | 30 ms | Pause after a complete key sequence |
| `after_text_delay` | 15 ms | Pause after typing plain text |

The total time per keystroke is `press_delay + release_delay + between_keys_delay` = 50 ms, which corresponds to roughly 240 WPM.

### Reference: Human Typing Speed Profiles

The table below shows realistic timing values for different typing speeds, based on keystroke dynamics research:

- [Monrose & Rubin (2000): "Keystroke dynamics as a biometric for authentication"](http://www1.cs.columbia.edu/4180/hw/keystroke.pdf)
- [Robinson et al. (1998): "Computer user verification using login string keystroke dynamics"](https://doi.org/10.1109/3468.661150)
- [Feit, Weir & Oulasvirta (2016): "How We Type: Movement Strategies and Performance in Everyday Typing" (CHI 2016)](https://userinterfaces.aalto.fi/how-we-type/)

Use these as a guide when adjusting timings to simulate human-like input.

#### Average typist (~50 WPM)

| Parameter | Value |
|-----------|-------|
| `press_delay` | 100 ms |
| `release_delay` | 15 ms |
| `between_keys_delay` | 125 ms |
| `chord_press_delay` | 50 ms |
| `chord_release_delay` | 50 ms |
| `after_sequence_delay` | 150 ms |
| `after_text_delay` | 75 ms |

#### Fast typist (~120 WPM)

| Parameter | Value |
|-----------|-------|
| `press_delay` | 50 ms |
| `release_delay` | 10 ms |
| `between_keys_delay` | 40 ms |
| `chord_press_delay` | 25 ms |
| `chord_release_delay` | 25 ms |
| `after_sequence_delay` | 60 ms |
| `after_text_delay` | 30 ms |

#### Test automation default (~240 WPM)

Same as the defaults listed above. Twice as fast as a fast human typist, but still with realistic pauses between events.

### Overriding Timing

If typing feels too slow, or an application drops characters, you can adjust the timing:

```bash
# CLI: set all delays to 10ms (faster)
platynui-cli keyboard type --delay-ms 10 "Hello World"
```

```python
# Python: override specific delays per call
rt.keyboard_type("Hello", overrides={"between_keys_delay_ms": 5.0})

# Python: change global defaults
rt.set_keyboard_settings({"between_keys_delay_ms": 25.0})
```
