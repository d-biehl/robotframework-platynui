#!/usr/bin/env python3
"""
Example: Using Mock Providers for Unit Testing

This example demonstrates how to use PlatynUI's mock providers to create
deterministic unit tests without requiring actual UI automation.
"""

import platynui_native as pn
from typing import List


def test_xpath_evaluation_with_mock_tree() -> None:
    """
    Demonstrate XPath evaluation against the mock provider's tree.

    The mock provider exposes a deterministic tree structure that includes
    typical desktop application elements (windows, buttons, text fields, etc.).
    """
    # Create a Runtime with only the mock provider (no OS-specific providers)
    rt = pn.Runtime.new_with_providers([pn.MOCK_PROVIDER])

    print("=" * 70)
    print("Testing XPath Evaluation with Mock Provider")
    print("=" * 70)

    # Get the desktop node
    desktop = rt.desktop_node()
    print(f"\nDesktop: {desktop.name} ({desktop.role})")

    # Query for all buttons
    # Evaluate returns a union of possible XDM values; filter to UiNode for safe access
    buttons: List[pn.UiNode] = [
        n for n in rt.evaluate("//Button") if isinstance(n, pn.UiNode)
    ]
    print(f"\nFound {len(buttons)} buttons:")
    for i, btn in enumerate(buttons, 1):
        print(f"  {i}. {btn.name}")

    # Query for elements with specific attributes
    focusable_elements: List[pn.UiNode] = [
        n for n in rt.evaluate("//*[@IsFocused='true']") if isinstance(n, pn.UiNode)
    ]
    print(f"\nFocused elements: {len(focusable_elements)}")
    for elem in focusable_elements:
        print(f"  - {elem.name} ({elem.role})")

    # Use evaluate_iter for lazy evaluation
    print("\nIterating over all elements:")
    count = 0
    for item in rt.evaluate_iter("//*"):
        if isinstance(item, pn.UiNode):
            count += 1
    print(f"  Total nodes: {count}")

    # Use evaluate_single to get just the first match
    first_window = rt.evaluate_single("//Window")
    if isinstance(first_window, pn.UiNode):
        print(f"\nFirst window: {first_window.name}")


def test_pointer_automation_with_mock() -> None:
    """
    Demonstrate pointer automation with mock devices.

    Mock pointer device logs all operations, making it perfect for
    verifying automation sequences in tests.
    """
    # Create platform overrides with mock pointer device
    platforms = pn.PlatformOverrides()
    platforms.pointer = pn.MOCK_POINTER_DEVICE

    # Create Runtime with mock provider and mock platforms
    rt = pn.Runtime.new_with_providers_and_platforms([pn.MOCK_PROVIDER], platforms)

    print("\n" + "=" * 70)
    print("Testing Pointer Automation with Mock Device")
    print("=" * 70)

    # Get initial position
    pos = rt.pointer_position()
    print(f"\nInitial pointer position: ({pos.x}, {pos.y})")

    # Move to a specific point
    target = pn.Point(100.0, 200.0)
    new_pos = rt.pointer_move_to(target)
    print(f"Moved to: ({new_pos.x}, {new_pos.y})")
    assert new_pos.x == 100.0 and new_pos.y == 200.0, "Position mismatch!"

    # Perform a click
    rt.pointer_click(pn.Point(150.0, 250.0))
    print("Clicked at (150, 250)")

    # Perform a drag operation
    rt.pointer_drag(pn.Point(50.0, 50.0), pn.Point(150.0, 150.0))
    print("Dragged from (50, 50) to (150, 150)")


def test_keyboard_automation_with_mock() -> None:
    """
    Demonstrate keyboard automation with mock devices.

    Mock keyboard device logs all key events, making it easy to verify
    keyboard sequences in tests.
    """
    # Create platform overrides with mock keyboard device
    platforms = pn.PlatformOverrides()
    platforms.keyboard = pn.MOCK_KEYBOARD_DEVICE

    rt = pn.Runtime.new_with_providers_and_platforms([pn.MOCK_PROVIDER], platforms)

    print("\n" + "=" * 70)
    print("Testing Keyboard Automation with Mock Device")
    print("=" * 70)

    # Type a simple string
    rt.keyboard_type("Hello, World!")
    print("\nTyped: 'Hello, World!'")

    # Press a key combination
    rt.keyboard_press("Ctrl+C")
    print("Pressed: Ctrl+C")

    # Release keys
    rt.keyboard_release("Ctrl+C")
    print("Released: Ctrl+C")


def test_complete_ui_test_scenario() -> None:
    """
    Demonstrate a complete UI test scenario using all mock providers.

    This shows how you can write deterministic UI tests that don't
    require actual applications running.
    """
    # Set up complete mock environment
    platforms = pn.PlatformOverrides()
    platforms.highlight = pn.MOCK_HIGHLIGHT_PROVIDER
    platforms.screenshot = pn.MOCK_SCREENSHOT_PROVIDER
    platforms.pointer = pn.MOCK_POINTER_DEVICE
    platforms.keyboard = pn.MOCK_KEYBOARD_DEVICE

    rt = pn.Runtime.new_with_providers_and_platforms([pn.MOCK_PROVIDER], platforms)

    print("\n" + "=" * 70)
    print("Complete UI Test Scenario")
    print("=" * 70)

    # 1. Find a UI element using XPath
    button = rt.evaluate_single("//Button[@Name='Submit']")
    if not isinstance(button, pn.UiNode):
        print("\n⚠️  'Submit' button not found in mock tree")
        # Try finding any button
        button = rt.evaluate_single("//Button")

    if isinstance(button, pn.UiNode):
        print(f"\n✓ Found button: {button.name}")

        # 2. Get the button's bounding rectangle
        bounds_value = button.attribute("Bounds", "control")
        if isinstance(bounds_value, pn.Rect):
            bounds = bounds_value
            print(
                f"  Bounds: x={bounds.x}, y={bounds.y}, "
                f"w={bounds.width}, h={bounds.height}"
            )

            # 3. Highlight the element (for debugging/visualization)
            rt.highlight([bounds])
            print("  ✓ Highlighted button")

            # 4. Move pointer to button center
            center_x = bounds.x + bounds.width / 2
            center_y = bounds.y + bounds.height / 2
            rt.pointer_move_to(pn.Point(center_x, center_y))
            print(f"  ✓ Moved pointer to button center ({center_x}, {center_y})")

            # 5. Click the button
            rt.pointer_click(pn.Point(center_x, center_y))
            print("  ✓ Clicked button")

            # 6. Clear highlight
            rt.clear_highlight()
            print("  ✓ Cleared highlight")

    # 7. Test keyboard input in a text field
    text_field = rt.evaluate_single("//Edit")
    if isinstance(text_field, pn.UiNode):
        print(f"\n✓ Found text field: {text_field.name}")

        # Type some text
        rt.keyboard_type("Test Input")
        print("  ✓ Typed: 'Test Input'")

        # Select all and delete
        rt.keyboard_press("Ctrl+A")
        rt.keyboard_type("{DELETE}")
        print("  ✓ Cleared text field")


if __name__ == "__main__":
    test_xpath_evaluation_with_mock_tree()
    test_pointer_automation_with_mock()
    test_keyboard_automation_with_mock()
    test_complete_ui_test_scenario()

    print("\n" + "=" * 70)
    print("✅ All examples completed successfully!")
    print("=" * 70)
    print("\nKey Takeaways:")
    print("  • Use MOCK_PROVIDER for deterministic UI tree testing")
    print("  • Use MOCK_*_DEVICE constants for platform device testing")
    print("  • Create Runtime with new_with_providers() or")
    print("    new_with_providers_and_platforms() for custom configurations")
    print("  • Mock providers enable fast, reliable unit tests")
    print("=" * 70)
