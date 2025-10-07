platynui-native
================

Native Python bindings for PlatynUI using PyO3 + maturin.

Quick start (local develop):

- uv sync --dev
- uv run maturin develop -m packages/native/Cargo.toml --release --features mock-provider

Then in Python:

```python
from platynui_native import core, runtime
pt = core.Point(1.0, 2.0)
rt = runtime.Runtime()
items = rt.evaluate("/control:Desktop", None)
```

Run smoke tests (uses mock provider feature):

```
uv run maturin develop -m packages/native/Cargo.toml --release --features mock-provider
uv run pytest -q packages/native/tests
```

API overview

- Module layout
  - `platynui_native.core`: `Point`/`Size`/`Rect`, `Namespace`, IDs, `attribute_names()`
  - `platynui_native.runtime`: `Runtime`, `UiNode`, `UiAttribute`, `EvaluatedAttribute`, pointer/keyboard methods

- Creating a Runtime
  - `rt = runtime.Runtime()`

- Pointer buttons
  - Accepts: `'left' | 'middle' | 'right'`, `runtime.PointerButton` enum, or `int`.
  - Mapping: `1 → LEFT`, `2 → MIDDLE`, `3 → RIGHT`, other ints → `Other(n)`.

- Pointer overrides (class)
  - Timing (milliseconds unless noted):
    - `after_move_delay_ms`, `after_input_delay_ms`, `press_release_delay_ms`,
      `after_click_delay_ms`, `before_next_click_delay_ms`, `multi_click_delay_ms`,
      `ensure_move_timeout_ms`, `scroll_delay_ms`, `max_move_duration_ms`.
    - `move_time_per_pixel_us` (microseconds per pixel)
  - Motion/profile:
    - `speed_factor: float`
    - `acceleration_profile: 'constant'|'ease_in'|'ease_out'|'smooth_step'`
  - Origin (choose one):
    - `'desktop'`
    - `core.Point(x, y)` → absolute desktop point
    - `core.Rect(x, y, w, h)` → relative to rect (top-left origin)
  - Scrolling:
    - `scroll_step: (h: float, v: float)`; `scroll_delay_ms`
  - Other:
    - `ensure_move_threshold: float`
  - Properties: all fields are exposed read‑only (e.g. `ov.speed_factor`, `ov.origin`, `ov.after_move_delay_ms`).

- Keyboard overrides (class, milliseconds)
  - `press_delay_ms`, `release_delay_ms`, `between_keys_delay_ms`,
    `chord_press_delay_ms`, `chord_release_delay_ms`,
    `after_sequence_delay_ms`, `after_text_delay_ms`.
  - Properties: `kov.press_delay_ms`, `kov.between_keys_delay_ms`, etc.

Examples

```python
from platynui_native import core, runtime
rt = runtime.Runtime()

# Move & click with overrides
ov = runtime.PointerOverrides(speed_factor=1.5, after_move_delay_ms=15, origin='desktop')
rt.pointer_move_to(core.Point(100, 200), overrides=ov)
rt.pointer_click(core.Point(100, 200), button=runtime.PointerButton.LEFT,
                 overrides=runtime.PointerOverrides(multi_click_delay_ms=240))

# Drag with relative origin
rt.pointer_drag(core.Point(10, 10), core.Point(180, 140), button=runtime.PointerButton.LEFT,
                overrides=runtime.PointerOverrides(origin=core.Rect(50, 60, 200, 200)))

# Keyboard with custom timings
rt.keyboard_type("Hello", overrides=runtime.KeyboardOverrides(between_keys_delay_ms=5))
rt.keyboard_press("<Ctrl+C>")
rt.keyboard_release("<Ctrl+C>")
```

Notes

- The mock provider feature (`--features mock-provider`) is intended for local development without platform backends. Some pointer/keyboard calls may raise `PointerError`/`KeyboardError` if the device is not available; structure/typing of arguments is still validated.

- Evaluate results
  - `Runtime.evaluate()` returns a list of `UiNode`, `EvaluatedAttribute`, or plain values (`UiValue`).
  - `UiNode.attributes()` returns `list[UiAttribute]` (no owner).
  - `EvaluatedAttribute` includes an `owner()` reference back to the `UiNode` it belongs to.

Reference

- Pointer defaults (from runtime profile/settings)
  - double_click_time: 500 ms; double_click_size: (4.0, 4.0); default_button: left
  - mode: Linear; steps_per_pixel: 1.5; speed_factor: 1.0
  - max_move_duration: 600 ms; move_time_per_pixel: 800 µs
  - acceleration_profile: SmoothStep
  - after_move_delay: 40 ms; after_input_delay: 35 ms
  - press_release_delay: 50 ms; after_click_delay: 80 ms
  - before_next_click_delay: 120 ms; multi_click_delay: 500 ms
  - ensure_move_position: true; ensure_move_threshold: 2.0; ensure_move_timeout: 250 ms
  - scroll_step: (0.0, -120.0); scroll_delay: 40 ms

- Keyboard defaults (KeyboardSettings.default)
  - press_delay: 35 ms; release_delay: 25 ms
  - between_keys_delay: 40 ms
  - chord_press_delay: 45 ms; chord_release_delay: 45 ms
  - after_sequence_delay: 75 ms; after_text_delay: 20 ms

Robot Framework usage (example)

Wrap the native runtime into a simple Python library and import it in Robot suites:

1) Create `examples/robot/platynui_lib.py`:

```python
from platynui_native import runtime

class PlatynUILib:
    def __init__(self):
        self.rt = runtime.Runtime()

    def pointer_move_to(self, x: float, y: float):
        self.rt.pointer_move_to((float(x), float(y)))

    def keyboard_type(self, sequence: str):
        self.rt.keyboard_type(sequence)
```

2) Create `examples/robot/quickstart.robot`:

```
*** Settings ***
Library    examples/robot/platynui_lib.py

*** Test Cases ***
Pointer And Keyboard Smoke
    Pointer Move To    100    200
    Keyboard Type      Hello
```

3) Run with the mock feature installed:

```
uv run maturin develop -m packages/native/Cargo.toml --release --features mock-provider
uv run robot examples/robot/quickstart.robot

Troubleshooting

## Mock Providers and Platform Devices

Mock providers do NOT auto-register in the inventory system. They are explicitly exposed as Python handles for testing:

### Available Mock Components

- **`MOCK_PROVIDER`** - Mock UI tree provider (provides a test UI tree)
- **`MOCK_PLATFORM`** - Mock desktop info provider (provides MockOS desktop with 3 monitors)
- **`MOCK_HIGHLIGHT_PROVIDER`** - Mock highlight provider
- **`MOCK_SCREENSHOT_PROVIDER`** - Mock screenshot provider
- **`MOCK_POINTER_DEVICE`** - Mock pointer/mouse device
- **`MOCK_KEYBOARD_DEVICE`** - Mock keyboard device

### Usage Examples

**Basic Runtime with Mock Provider:**
```python
from platynui_native import Runtime, MOCK_PROVIDER

rt = Runtime.new_with_providers([MOCK_PROVIDER])
# Uses OS platform devices (may fail if not available)
```

**Full Mock Runtime (for testing):**
```python
from platynui_native import (
    Runtime, PlatformOverrides,
    MOCK_PROVIDER, MOCK_PLATFORM,
    MOCK_POINTER_DEVICE, MOCK_KEYBOARD_DEVICE,
    MOCK_SCREENSHOT_PROVIDER, MOCK_HIGHLIGHT_PROVIDER
)

# Configure platform overrides
overrides = PlatformOverrides()
overrides.desktop_info = MOCK_PLATFORM
overrides.pointer = MOCK_POINTER_DEVICE
overrides.keyboard = MOCK_KEYBOARD_DEVICE
overrides.screenshot = MOCK_SCREENSHOT_PROVIDER
overrides.highlight = MOCK_HIGHLIGHT_PROVIDER

# Create runtime with all mock components
rt = Runtime.new_with_providers_and_platforms([MOCK_PROVIDER], overrides)

# Now all operations work without real OS providers
info = rt.desktop_info()
print(info["os_name"])  # Output: "MockOS"
```

### Build Requirements

The `mock-provider` feature links the mock crates, making the handles available:

```bash
uv run maturin develop -m packages/native/Cargo.toml --features mock-provider
```

### Common Issues

- **"no PointerDevice registered"**: Pass `MOCK_POINTER_DEVICE` in `PlatformOverrides`
- **"no KeyboardDevice registered"**: Pass `MOCK_KEYBOARD_DEVICE` in `PlatformOverrides`
- **Want mock desktop info**: Pass `MOCK_PLATFORM` in `overrides.desktop_info`

## Other Issues

- If `maturin` complains about the manifest path, always pass `-m packages/native/Cargo.toml` for mixed projects.
- If your shell shows a VIRTUAL_ENV mismatch, prefer `uv run --active ...` to target the active environment explicitly.
```
