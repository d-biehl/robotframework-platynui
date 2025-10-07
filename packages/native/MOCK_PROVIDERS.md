# Using Mock Providers for Testing

PlatynUI provides mock providers that enable deterministic, fast unit tests without requiring actual applications or OS-specific UI automation APIs.

## Mock Provider Availability

Mock providers are **always available** in the Python bindings as explicit handles - no special build flags or features are required. Unlike OS-specific providers (AT-SPI, Windows UIA, macOS AX) which auto-register when linked, mock providers are **only available via explicit handles** and never appear in the default `Runtime()` discovery. This design ensures:

- Production code never accidentally uses mock providers
- Tests have full control over which providers are active
- Clean separation between OS providers (auto-discovered) and test mocks (explicit-only)

## Mock Provider Exports

The following constants are always available:

- **`MOCK_PROVIDER`**: Handle for the mock UiTreeProvider factory
- **`MOCK_PLATFORM`**: Handle for mock desktop info provider (provides MockOS with 3 monitors)
- **`MOCK_HIGHLIGHT_PROVIDER`**: Handle for mock highlight provider
- **`MOCK_SCREENSHOT_PROVIDER`**: Handle for mock screenshot provider
- **`MOCK_POINTER_DEVICE`**: Handle for mock pointer device
- **`MOCK_KEYBOARD_DEVICE`**: Handle for mock keyboard device

These are opaque integer handles that can be passed to Runtime constructors.

## Creating Test Runtimes

### Basic Mock Provider

Create a Runtime with only the mock provider (no OS-specific providers):

```python
import platynui_native as pn

# Create Runtime with mock provider only
rt = pn.Runtime.new_with_providers([pn.MOCK_PROVIDER])

# Now you can query the deterministic mock tree
buttons = rt.evaluate("//Button")
print(f"Found {len(buttons)} buttons in mock tree")
```

### Mock Platform Providers

Create a Runtime with mock platform devices for testing automation sequences:

```python
import platynui_native as pn

# Configure platform overrides
platforms = pn.PlatformOverrides()
platforms.desktop_info = pn.MOCK_PLATFORM  # Mock desktop with 3 monitors
platforms.pointer = pn.MOCK_POINTER_DEVICE
platforms.keyboard = pn.MOCK_KEYBOARD_DEVICE

# Create Runtime with mock provider and mock platforms
rt = pn.Runtime.new_with_providers_and_platforms(
    [pn.MOCK_PROVIDER],
    platforms
)

# Access mock desktop info
info = rt.desktop_info()
print(f"OS: {info['os_name']}")  # Output: "MockOS"
print(f"Monitors: {info['display_count']}")  # Output: 3

# Test pointer automation
rt.pointer_move_to(pn.Point(100.0, 200.0))
rt.pointer_click(pn.Point(100.0, 200.0))

# Test keyboard automation
rt.keyboard_type("Hello World")
```

### Complete Mock Environment

For comprehensive testing, set up all mock providers:

```python
import platynui_native as pn

platforms = pn.PlatformOverrides()
platforms.desktop_info = pn.MOCK_PLATFORM
platforms.highlight = pn.MOCK_HIGHLIGHT_PROVIDER
platforms.screenshot = pn.MOCK_SCREENSHOT_PROVIDER
platforms.pointer = pn.MOCK_POINTER_DEVICE
platforms.keyboard = pn.MOCK_KEYBOARD_DEVICE

rt = pn.Runtime.new_with_providers_and_platforms(
    [pn.MOCK_PROVIDER],
    platforms
)

# Now you can test complete UI automation workflows
button = rt.evaluate_single("//Button")
bounds = button.attribute("Bounds", "control")
rt.highlight([bounds])  # Visual debugging
rt.pointer_click(pn.Point(bounds.x + bounds.width/2, bounds.y + bounds.height/2))
rt.clear_highlight()
```

## Mock Tree Structure

The mock provider exposes a deterministic tree with common UI elements:

- Desktop node
- Window with various controls
- Buttons, text fields, checkboxes
- Standard attributes (Bounds, IsFocused, etc.)

Query it using XPath expressions as you would with real applications.

## Benefits for Testing

1. **Speed**: No need to launch actual applications
2. **Determinism**: Same tree structure every time
3. **Isolation**: Tests don't interfere with each other
4. **Debugging**: Mock devices log all operations
5. **CI/CD**: Run UI tests in headless environments

## Example: Complete Test Scenario

```python
def test_button_click_workflow():
    """Test a complete button click workflow with mocks."""
    platforms = pn.PlatformOverrides()
    platforms.pointer = pn.MOCK_POINTER_DEVICE
    platforms.highlight = pn.MOCK_HIGHLIGHT_PROVIDER

    rt = pn.Runtime.new_with_providers_and_platforms(
        [pn.MOCK_PROVIDER],
        platforms
    )

    # Find button using XPath
    button = rt.evaluate_single("//Button[@Name='OK']")
    assert button is not None, "OK button should exist"

    # Get bounds
    bounds = button.attribute("Bounds", "control")
    assert isinstance(bounds, pn.Rect)

    # Highlight for visual feedback (useful during debugging)
    rt.highlight([bounds])

    # Click button center
    center_x = bounds.x + bounds.width / 2
    center_y = bounds.y + bounds.height / 2
    rt.pointer_click(pn.Point(center_x, center_y))

    # Clear highlight
    rt.clear_highlight()

    print("✅ Button click workflow completed")
```

## See Also

- `test_mock_providers.py`: Basic mock provider tests
- `examples/mock_providers_example.py`: Comprehensive examples
- Rust documentation: `crates/provider-mock/` and `crates/platform-mock/`
