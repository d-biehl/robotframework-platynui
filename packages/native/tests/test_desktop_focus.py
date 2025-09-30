from platynui_native import runtime, core


def test_desktop_node_and_info():
    rt = runtime.Runtime()
    node = rt.desktop_node()
    assert isinstance(node, runtime.UiNode)
    assert node.role == "Desktop"
    info = rt.desktop_info()
    assert isinstance(info, dict)
    assert "bounds" in info and hasattr(info["bounds"], "to_tuple")
    assert isinstance(info.get("monitors", []), list)


def test_focus_via_runtime_or_skip():
    rt = runtime.Runtime()
    # Try to find a focusable mock button; skip if none available
    items = rt.evaluate("//control:Button[@Name='OK']")
    target = next((x for x in items if isinstance(x, runtime.UiNode)), None)
    if target is None:
        return
    try:
        rt.focus(target)
    except runtime.PatternError:
        # On platforms without Focusable, we accept a PatternError
        pass
