# Pointer Input

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

PlatynUI provides a rich pointer (mouse) automation API — consistent across CLI, Python, and Robot Framework. It supports moving, clicking, double-clicking, scrolling, dragging, and pressing/releasing buttons with configurable motion paths, timing, and acceleration.

## 1. Overview

Every pointer action follows the same pattern:

1. **Move** the pointer to a target (element or coordinates).
2. **Act** — click, press, release, scroll, or drag.
3. **Wait** — small configurable pauses let the application process the input.

PlatynUI automatically resolves element positions via the UI tree: it prefers `@ActivationPoint` when available, otherwise uses the center of `@Bounds`.

## 2. Actions

### Move

Moves the pointer from its current position to the target using the active motion path.

```bash
platynui-cli pointer move "//control:Button[@Name='OK']"
platynui-cli pointer move --point 400,300
```

### Click

Single-clicks at the target. Moves to the target first unless `--no-move` is specified.

```bash
platynui-cli pointer click "//control:Button[@Name='OK']"
platynui-cli pointer click --point 400,300
platynui-cli pointer click --button right "//control:ListItem[@Name='File']"
```

### Multi-Click

Performs multiple rapid clicks (e.g. double-click, triple-click). PlatynUI automatically spaces clicks within the system's double-click time window.

```bash
platynui-cli pointer multi-click "//control:Text[@Name='word']"
platynui-cli pointer multi-click --count 3 "//control:Text[@Name='paragraph']"
```

### Press / Release

Presses or releases a button independently — useful for drag gestures or hold-and-click scenarios.

```bash
platynui-cli pointer press "//control:Slider"
platynui-cli pointer release --button left
```

### Scroll

Scrolls by a delta value at the current position or at a target element.

```bash
platynui-cli pointer scroll 0,-360               # scroll down 3 notches
platynui-cli pointer scroll 0,120 --expr "//control:List"
```

The delta is specified as `horizontal,vertical`. One notch on a typical scroll wheel is ±120 units.

### Drag

Drags from one point to another while holding a button.

```bash
platynui-cli pointer drag --from 100,200 --to 300,400
platynui-cli pointer drag --from 100,200 --to 300,400 --button left
```

### Position

Prints the current pointer location.

```bash
platynui-cli pointer position
```

## 3. Buttons

| Name | Aliases | Description |
|------|---------|-------------|
| `left` | `primary` | Default button |
| `right` | `secondary` | Context menu |
| `middle` | `wheel` | Middle/wheel button |
| *numeric code* | — | Platform-specific extra buttons (e.g. `4`, `5`) |

## 4. Motion Modes

The motion mode controls how the pointer path is generated between start and target.

| Mode | Description |
|------|-------------|
| `Direct` | Teleports to the target in a single OS call — no intermediate steps. The calculated duration is still waited. |
| `Linear` | Moves in a straight line from start to target. This is the **default**. |
| `Bezier` | Follows a curved path (quadratic Bézier). The curve bulges perpendicular to the direct path; `curve_amplitude` controls how far. |
| `Overshoot` | Moves past the target, then settles back. `overshoot_ratio` controls how far past (fraction of total distance); `overshoot_settle_steps` controls the number of correction steps. |
| `Jitter` | Adds a sinusoidal wobble perpendicular to the path. `jitter_amplitude` controls the wobble height; `jitter_frequency` controls how many oscillation cycles occur along the path. A sine-envelope tapers the wobble to zero at start and end. |

## 5. Acceleration Profiles

The acceleration profile determines how motion steps are distributed over time. It changes *when* the pointer pauses between steps — not the path shape.

| Profile | Description |
|---------|-------------|
| `Constant` | Even spacing — each step takes the same time. |
| `EaseIn` | Starts slow, accelerates towards the target. Step durations grow quadratically. |
| `EaseOut` | Starts fast, decelerates near the target. Step durations shrink. |
| `SmoothStep` | Slow start, fast middle, slow end (Hermite interpolation). This is the **default**. |

## 6. How Move Duration Is Calculated

The total duration of a pointer move is determined by two independent mechanisms. Whichever produces the *smaller* value wins:

### Distance-based duration

```
base_duration = move_time_per_pixel × (distance / speed_factor)
```

- `move_time_per_pixel` — time per pixel of travel (default: 800 µs).
- `speed_factor` — multiplier on the effective distance.
  - `1.0` = normal speed (default).
  - `2.0` = pointer arrives in half the time (twice as fast).
  - `0.5` = pointer takes twice as long (half as fast).
  - Values ≤ 0 are treated as `1.0`.

### Maximum duration cap

```
final_duration = min(base_duration, max_move_duration)
```

- `max_move_duration` — hard upper bound (default: 600 ms).

**Important:** If `max_move_duration` is set too low, reducing `speed_factor` below 1.0 won't make the pointer visibly slower — the cap will kick in. To see a slower pointer, either increase `max_move_duration` or set it to `0` (no cap).

If *both* `move_time_per_pixel` and `max_move_duration` are zero, the path is traversed as fast as the OS allows (no intentional delays between steps).

### Path resolution

Regardless of timing, the number of intermediate points on the path is:

```
steps = ceil(distance × steps_per_pixel)
```

- `steps_per_pixel` — points per pixel of travel (default: 1.5). Higher values yield smoother motion but more OS calls.

## 7. Timing Parameters

PlatynUI inserts small pauses at key moments so applications can process pointer events reliably.

### Profile Timing (long-lived defaults)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `move_time_per_pixel` | 800 µs | Time per pixel of travel. Determines the base move duration together with `speed_factor`. |
| `max_move_duration` | 600 ms | Hard upper bound on move duration. `0` = no limit. |
| `speed_factor` | 1.0 | Divides the effective distance: `>1` = faster, `<1` = slower, `≤0` treated as `1.0`. |
| `steps_per_pixel` | 1.5 | Number of intermediate path points per pixel. |
| `after_move_delay` | 40 ms | Pause after the pointer finishes moving. |
| `after_input_delay` | 35 ms | Pause after any pointer input action (click, scroll, etc.). |
| `press_release_delay` | 50 ms | How long a button is held down during a click. |
| `after_click_delay` | 80 ms | Pause after completing a click. |
| `before_next_click_delay` | 120 ms | Minimum pause before a subsequent click on the same target. |
| `multi_click_delay` | 500 ms | Maximum time between clicks to consider them part of the same group (e.g. double-click). |
| `ensure_move_position` | `true` | Verify the pointer actually arrived at the target after moving. |
| `ensure_move_threshold` | 2.0 px | Acceptable deviation when verifying position. |
| `ensure_move_timeout` | 250 ms | How long to retry reaching the target before failing. |

### Motion Shape Parameters

| Parameter | Default | Used by | Description |
|-----------|---------|---------|-------------|
| `curve_amplitude` | 40.0 | Bezier | How far the curve bulges from the straight-line path (pixels). |
| `overshoot_ratio` | 0.20 | Overshoot | Fraction of total distance to overshoot past the target. |
| `overshoot_settle_steps` | 3 | Overshoot | Number of correction steps to settle back on target. |
| `jitter_amplitude` | 8.0 | Jitter | Sinusoidal wobble amplitude perpendicular to the path (pixels). |
| `jitter_frequency` | 6.0 | Jitter | Number of full sine-wave cycles along the path. Higher values produce faster wobble. |

### Scroll Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `scroll_step` | `(0, -120)` | Delta per step. One notch on a typical mouse wheel = 120 units. |
| `scroll_delay` | 40 ms | Pause between scroll steps. |

### Pointer Settings (system-level)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `double_click_time` | 500 ms | Maximum interval between two clicks for them to count as a double-click. |
| `double_click_size` | 4×4 px | Maximum pointer drift between clicks for them to count as a double-click. |
| `default_button` | Left | Button used when none is specified. |

## 8. Adjusting Behaviour

### Globally (Profile)

Change the pointer profile to affect all subsequent actions:

```python
from platynui_native import Runtime

rt = Runtime()

# Switch to a slower, curved motion
rt.set_pointer_profile({
    "motion": "bezier",
    "speed_factor": 0.5,
    "max_move_duration_ms": 2000,
    "curve_amplitude": 60.0,
})

# Switch to instant teleport with no delays
rt.set_pointer_profile({
    "motion": "direct",
    "move_time_per_pixel_us": 0,
    "max_move_duration_ms": 0,
    "after_move_delay_ms": 0,
    "after_input_delay_ms": 0,
    "after_click_delay_ms": 0,
})
```

You can also construct a `PointerProfile` object:

```python
from platynui_native import PointerProfile

profile = PointerProfile(
    motion="linear",
    speed_factor=2.0,
    after_click_delay_ms=20,
)
rt.set_pointer_profile(profile)
```

### Per Action (Overrides)

Override individual parameters for a single call without changing the global profile:

```python
from platynui_native import PointerOverrides

# Click with fast motion for this call only
rt.pointer_click(
    point,
    overrides=PointerOverrides(speed_factor=3.0, motion="direct"),
)

# Or use a dictionary
rt.pointer_move_to(point, overrides={"speed_factor": 0.3, "max_move_duration_ms": 3000})
```

### System Settings

```python
# Adjust double-click timing
rt.set_pointer_settings({"double_click_time_ms": 400})
```

## 9. Usage

### CLI

```bash
# Move to an element
platynui-cli pointer move "//control:Button[@Name='Sign in']"

# Click at coordinates
platynui-cli pointer click --point 500,300

# Double-click on an element
platynui-cli pointer multi-click "//control:ListItem[@Name='Document']"

# Right-click
platynui-cli pointer click --button right "//control:ListItem[@Name='File']"

# Scroll down
platynui-cli pointer scroll 0,-360

# Drag from one point to another
platynui-cli pointer drag --from 100,200 --to 400,500

# Override motion and speed
platynui-cli pointer move --point 800,400 --motion bezier --speed-factor 0.5

# Check current position
platynui-cli pointer position
```

#### CLI Override Flags

All pointer subcommands accept these optional flags:

| Flag | Description |
|------|-------------|
| `--motion <mode>` | Motion mode: `direct`, `linear`, `bezier`, `overshoot`, `jitter` |
| `--speed-factor <f64>` | Speed multiplier |
| `--acceleration <profile>` | Acceleration: `constant`, `ease-in`, `ease-out`, `smooth-step` |
| `--move-duration <ms>` | Maximum move duration in milliseconds |
| `--move-time-per-pixel <ms>` | Time per pixel in milliseconds |
| `--after-move <ms>` | Pause after move |
| `--after-input <ms>` | Pause after input action |
| `--press-release <ms>` | Button hold time |
| `--after-click <ms>` | Pause after click |
| `--before-next <ms>` | Pause before next click |
| `--multi-click <ms>` | Multi-click grouping window |
| `--scroll-step <h,v>` | Delta per scroll step |
| `--scroll-delay <ms>` | Pause between scroll steps |
| `--ensure-threshold <px>` | Position verification threshold |
| `--ensure-timeout <ms>` | Position verification timeout |
| `--origin <kind>` | Coordinate origin: `desktop`, `bounds`, `absolute` |
| `--no-activate` | Skip bringing the target window to the foreground |

### Python

```python
from platynui_native import Runtime, PointerProfile, PointerOverrides
from platynui_native.runtime import Point

rt = Runtime()

# Move to a point
rt.pointer_move_to(Point(500, 300))

# Click on an element found via XPath
button = rt.evaluate_single("//control:Button[@Name='OK']")
if button:
    rt.pointer_click(button.attribute("ActivationPoint"))

# Double-click
rt.pointer_multi_click(Point(200, 100), clicks=2)

# Scroll
rt.pointer_scroll((0, -360))

# Drag
rt.pointer_drag(Point(100, 200), Point(400, 500))

# Press and release independently
rt.pointer_press(Point(100, 100))
rt.pointer_release(Point(400, 400))

# Read current position
pos = rt.pointer_position()
print(f"Pointer at {pos.x}, {pos.y}")

# Change global profile
rt.set_pointer_profile({"speed_factor": 0.5, "motion": "bezier"})

# Override per call
rt.pointer_click(
    Point(300, 200),
    overrides={"speed_factor": 3.0, "after_click_delay_ms": 10},
)
```

### Robot Framework

```robotframework
*** Settings ***
Library    PlatynUI.BareMetal

*** Test Cases ***
Click A Button
    Pointer Click    //control:Button[@Name="OK"]

Click At Coordinates
    Pointer Click    x=${500}    y=${300}

Click Relative To Element
    Pointer Click    //control:Canvas    x=${10}    y=${20}

Double Click
    Pointer Multi Click    //control:ListItem[@Name="Document"]

Right Click
    Pointer Click    //control:ListItem[@Name="File"]    button=right

Move To Element
    Pointer Move To    //control:Button[@Name="Submit"]

Press And Release
    Pointer Press    //control:Slider    x=${0}    y=${5}
    Pointer Release    //control:Slider    x=${100}    y=${5}

Get Position
    ${pos}=    Get Pointer Position
    Log    Pointer at ${pos.x}, ${pos.y}
```

The first argument (`descriptor`) targets an element via XPath. PlatynUI resolves its screen coordinates automatically. Pass coordinates directly via `x`/`y` instead, or combine both (coordinates become offsets relative to the element's bounds).

### Configuring the Library at Import

```robotframework
*** Settings ***
Library    PlatynUI.BareMetal
...    pointer_profile=${{{"speed_factor": 0.5, "motion": "bezier"}}}
...    pointer_settings=${{{"double_click_time_ms": 400}}}
```

## 10. Understanding the Click Timeline

A single click goes through these phases:

```
[move to target] → after_move_delay
                 → [press button] → press_release_delay → [release button]
                 → after_click_delay → after_input_delay
```

For multi-click (e.g. double-click):

```
[move to target] → after_move_delay
                 → [press] → press_release_delay → [release]
                 → inter_click_pause (double_click_time / 2)
                 → [press] → press_release_delay → [release]
                 → after_click_delay → after_input_delay
```

When two separate `pointer_click` calls happen in quick succession on the same position, PlatynUI automatically enforces `before_next_click_delay` to avoid accidentally triggering a double-click — unless the `multi_click_delay` window has expired or the pointer has moved outside `double_click_size`.

## 11. Reference: Speed Profiles

The table below shows example configurations for different use cases:

### Test automation (fast, default)

| Parameter | Value |
|-----------|-------|
| `motion` | Linear |
| `speed_factor` | 1.0 |
| `move_time_per_pixel` | 800 µs |
| `max_move_duration` | 600 ms |
| `after_move_delay` | 40 ms |
| `press_release_delay` | 50 ms |
| `after_click_delay` | 80 ms |

### Instant (no animation)

| Parameter | Value |
|-----------|-------|
| `motion` | Direct |
| `move_time_per_pixel` | 0 |
| `max_move_duration` | 0 |
| `after_move_delay` | 0 |
| `after_input_delay` | 0 |
| `press_release_delay` | 10 ms |
| `after_click_delay` | 10 ms |

### Human-like (slow, curved)

| Parameter | Value |
|-----------|-------|
| `motion` | Bezier |
| `speed_factor` | 0.3 |
| `acceleration_profile` | SmoothStep |
| `max_move_duration` | 2000 ms |
| `move_time_per_pixel` | 2000 µs |
| `curve_amplitude` | 60.0 |
| `after_move_delay` | 80 ms |
| `press_release_delay` | 100 ms |
| `after_click_delay` | 150 ms |

### Demonstration / recording (very slow)

| Parameter | Value |
|-----------|-------|
| `motion` | Bezier |
| `speed_factor` | 0.15 |
| `acceleration_profile` | SmoothStep |
| `max_move_duration` | 5000 ms |
| `move_time_per_pixel` | 4000 µs |
| `curve_amplitude` | 80.0 |
| `after_move_delay` | 200 ms |
| `press_release_delay` | 200 ms |
| `after_click_delay` | 300 ms |
