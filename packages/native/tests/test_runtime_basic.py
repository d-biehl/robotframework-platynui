import math
import typing as t

import pytest


def is_num(x: t.Any) -> bool:
    return isinstance(x, (int, float)) and math.isfinite(float(x))


def is_rect_tuple(v: t.Any) -> bool:
    return (
        isinstance(v, tuple)
        and len(v) == 4
        and all(is_num(n) for n in v)
    )


def test_evaluate_desktop_node():
    from platynui_native import core, runtime

    rt = runtime.Runtime()
    items = rt.evaluate("/")
    assert isinstance(items, list)

    # find first Node in results
    node = next((it for it in items if isinstance(it, runtime.Node)), None)
    assert node is not None, "expected at least one Node result"

    assert node.role == "Desktop"
    assert node.namespace.as_str() == "control"

    # Bounds should be a 4-tuple (x, y, w, h)
    bounds = node.attribute("Bounds")
    assert is_rect_tuple(bounds), f"unexpected Bounds format: {bounds!r}"


def test_pointer_and_keyboard_smoke():
    from platynui_native import runtime

    rt = runtime.Runtime()

    # pointer_position may not be available on all platforms/mocks
    try:
        pos = rt.pointer_position()
        assert hasattr(pos, "x") and hasattr(pos, "y")
        # move back to same position to exercise conversion (tuple)
        rt.pointer_move_to((pos.x, pos.y))
    except runtime.PointerError:
        pytest.skip("Pointer device not available in this build")

    # keyboard may also be unavailable; calling should either succeed or raise KeyboardError
    try:
        rt.keyboard_type("a")
    except runtime.KeyboardError:
        pytest.skip("Keyboard device not available in this build")
