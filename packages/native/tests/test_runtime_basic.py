import math
import typing as t

from platynui_native import Rect, Runtime, UiNode


def is_num(x: t.Any) -> bool:
    return isinstance(x, (int, float)) and math.isfinite(float(x))


def test_evaluate_desktop_node(rt_mock_platform: Runtime) -> None:
    items = rt_mock_platform.evaluate('/')
    assert isinstance(items, list)

    # find first UiNode in results
    node = next((it for it in items if isinstance(it, UiNode)), None)
    assert node is not None, 'expected at least one UiNode result'

    assert node.role == 'Desktop'
    assert node.namespace.as_str() == 'control'

    # Bounds should be a Rect object with to_tuple() method
    bounds = node.attribute('Bounds')
    assert isinstance(bounds, Rect), f'Expected Rect, got {type(bounds)}'
    bounds_tuple = bounds.to_tuple()
    assert isinstance(bounds_tuple, tuple)
    assert len(bounds_tuple) == 4
    assert all(is_num(n) for n in bounds_tuple)


def test_pointer_and_keyboard_smoke(rt_mock_platform: Runtime) -> None:
    # pointer should be available with mock platform
    pos = rt_mock_platform.pointer_position()
    assert hasattr(pos, 'x')
    assert hasattr(pos, 'y')
    # move back to same position using Point
    rt_mock_platform.pointer_move_to(pos)

    # keyboard should be available with mock platform
    rt_mock_platform.keyboard_type('a')


def test_pointer_like_args(rt_mock_platform: Runtime) -> None:
    # tuple input
    pos = rt_mock_platform.pointer_move_to((10.0, 20.0))
    assert hasattr(pos, 'x')
    assert hasattr(pos, 'y')
    # dict input
    pos = rt_mock_platform.pointer_move_to({'x': 30, 'y': 40})
    assert pos.x == 30
    assert pos.y == 40
    # click accepts None (no move) and like types
    rt_mock_platform.pointer_click((5.0, 6.0))
    rt_mock_platform.pointer_click({'x': 7, 'y': 8})
    # drag accepts like types
    rt_mock_platform.pointer_drag((1.0, 2.0), (3.0, 4.0))
