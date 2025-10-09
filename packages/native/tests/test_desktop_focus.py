from platynui_native import (
    UiNode,
    Runtime,
)


def test_desktop_node_and_info(rt_mock_platform: Runtime) -> None:
    node = rt_mock_platform.desktop_node()
    assert isinstance(node, UiNode)
    assert node.role == "Desktop"
    info = rt_mock_platform.desktop_info()
    assert isinstance(info, dict)
    assert "bounds" in info and hasattr(info["bounds"], "to_tuple")
    # Mock platform exposes deterministic OS info
    assert info.get("os_name") == "MockOS"
    monitors = info.get("monitors", [])
    assert isinstance(monitors, list)
    assert len(monitors) == 3


def test_focus_sets_is_focused_flags(rt_mock_platform: Runtime) -> None:
    # Deterministic element from mock tree
    btn = rt_mock_platform.evaluate_single("//control:Button[@Name='OK']")
    assert isinstance(btn, UiNode)

    # Before focusing, IsFocused may be false; main assertion happens after focus

    # Focus should not raise and should toggle focus state on the node/window
    rt_mock_platform.focus(btn)

    # Refresh node state to ensure dynamic attribute reflects new focus
    btn.invalidate()
    after_btn = bool(btn.attribute("IsFocused", "control"))
    assert after_btn is True

    # The containing window should also report IsFocused true
    window = rt_mock_platform.evaluate_single("ancestor-or-self::control:Window", btn)
    assert isinstance(window, UiNode)
    window.invalidate()
    after_win = bool(window.attribute("IsFocused", "control"))
    assert after_win is True
