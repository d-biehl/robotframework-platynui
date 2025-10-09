#!/usr/bin/env python3
"""Test Mock Providers for unit testing."""

import platynui_native as pn
from platynui_native import Runtime


def test_mock_providers_available():
    """Test that mock provider constants are available."""
    # Check that they are integers (directly access them since they're always available)
    assert isinstance(pn.MOCK_PROVIDER, int)
    assert isinstance(pn.MOCK_HIGHLIGHT_PROVIDER, int)
    assert isinstance(pn.MOCK_SCREENSHOT_PROVIDER, int)
    assert isinstance(pn.MOCK_POINTER_DEVICE, int)
    assert isinstance(pn.MOCK_KEYBOARD_DEVICE, int)



def test_runtime_with_mock_provider(rt_mock_platform: Runtime) -> None:
    """Test creating a Runtime with only the mock provider."""
    # Verify it works
    providers = rt_mock_platform.providers()
    for _ in enumerate(providers, 1):
        pass

    # Should have exactly one provider (the mock provider)
    assert len(providers) == 1
    assert providers[0]["technology"] == "Mock"

    # Test that we can query the tree
    desktop = rt_mock_platform.desktop_node()
    assert desktop is not None


def test_runtime_with_mock_platforms(rt_mock_platform: Runtime) -> None:
    """Test creating a Runtime with mock platform providers."""

    # Test pointer
    try:
        rt_mock_platform.pointer_position()

        # Move pointer
        new_pos = rt_mock_platform.pointer_move_to(pn.Point(100.0, 200.0))
        assert new_pos.x == 100.0
        assert new_pos.y == 200.0
    except pn.PointerError:
        pass

    # Test keyboard
    try:
        rt_mock_platform.keyboard_type("Hello World")
    except pn.KeyboardError:
        pass

    # Test highlight
    try:
        rt_mock_platform.highlight([pn.Rect(10.0, 10.0, 100.0, 100.0)])
        rt_mock_platform.clear_highlight()
    except Exception:
        pass


if __name__ == "__main__":
    pass
