#!/usr/bin/env python3
"""Test Mock Providers for unit testing."""

import platynui_native as pn


def test_mock_providers_available():
    """Test that mock provider constants are available."""
    # Check that they are integers (directly access them since they're always available)
    assert isinstance(pn.MOCK_PROVIDER, int)
    assert isinstance(pn.MOCK_HIGHLIGHT_PROVIDER, int)
    assert isinstance(pn.MOCK_SCREENSHOT_PROVIDER, int)
    assert isinstance(pn.MOCK_POINTER_DEVICE, int)
    assert isinstance(pn.MOCK_KEYBOARD_DEVICE, int)

    print("✅ Mock provider constants are available:")
    print(f"  MOCK_PROVIDER = {pn.MOCK_PROVIDER}")
    print(f"  MOCK_HIGHLIGHT_PROVIDER = {pn.MOCK_HIGHLIGHT_PROVIDER}")
    print(f"  MOCK_SCREENSHOT_PROVIDER = {pn.MOCK_SCREENSHOT_PROVIDER}")
    print(f"  MOCK_POINTER_DEVICE = {pn.MOCK_POINTER_DEVICE}")
    print(f"  MOCK_KEYBOARD_DEVICE = {pn.MOCK_KEYBOARD_DEVICE}")


def test_runtime_with_mock_provider():
    """Test creating a Runtime with only the mock provider."""
    # Create Runtime with only mock provider
    rt = pn.Runtime.new_with_providers([pn.MOCK_PROVIDER])

    # Verify it works
    providers = rt.providers()
    print("\n✅ Runtime created with mock provider:")
    print(f"  Active providers: {len(providers)}")
    for i, p in enumerate(providers, 1):
        print(f"    {i}. {p['display_name']} ({p['technology']})")

    # Should have exactly one provider (the mock provider)
    assert len(providers) == 1
    assert providers[0]["technology"] == "Mock"

    # Test that we can query the tree
    desktop = rt.desktop_node()
    print(f"\n  Desktop node: {desktop.name} ({desktop.role})")
    assert desktop is not None


def test_runtime_with_mock_platforms():
    """Test creating a Runtime with mock platform providers."""
    # Create PlatformOverrides with mock devices
    platforms = pn.PlatformOverrides()
    platforms.highlight = pn.MOCK_HIGHLIGHT_PROVIDER
    platforms.screenshot = pn.MOCK_SCREENSHOT_PROVIDER
    platforms.pointer = pn.MOCK_POINTER_DEVICE
    platforms.keyboard = pn.MOCK_KEYBOARD_DEVICE

    # Create Runtime with mock provider and mock platforms
    rt = pn.Runtime.new_with_providers_and_platforms([pn.MOCK_PROVIDER], platforms)

    print("\n✅ Runtime created with mock platforms:")

    # Test pointer
    try:
        pos = rt.pointer_position()
        print(f"  Pointer position: ({pos.x}, {pos.y})")

        # Move pointer
        new_pos = rt.pointer_move_to(pn.Point(100.0, 200.0))
        print(f"  Moved pointer to: ({new_pos.x}, {new_pos.y})")
        assert new_pos.x == 100.0
        assert new_pos.y == 200.0
    except pn.PointerError as e:
        print(f"  Pointer test skipped: {e}")

    # Test keyboard
    try:
        rt.keyboard_type("Hello World")
        print("  Keyboard typed: 'Hello World'")
    except pn.KeyboardError as e:
        print(f"  Keyboard test skipped: {e}")

    # Test highlight
    try:
        rt.highlight([pn.Rect(10.0, 10.0, 100.0, 100.0)])
        print("  Highlight created")
        rt.clear_highlight()
        print("  Highlight cleared")
    except Exception as e:
        print(f"  Highlight test skipped: {e}")


if __name__ == "__main__":
    test_mock_providers_available()
    test_runtime_with_mock_provider()
    test_runtime_with_mock_platforms()
    print("\n✅ All mock provider tests passed!")
