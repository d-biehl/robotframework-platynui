from collections.abc import Callable
from typing import Any

import pytest
from platynui_native import Runtime


def _assert_frompyobject_ok(call: Callable[[], Any]) -> None:
    try:
        call()
    except Exception as e:
        # Accept device errors; conversion succeeded if class name matches
        if type(e).__name__ not in {'PointerError', 'KeyboardError'}:
            raise


def test_pointer_overrides_accept_origin_desktop_point_rect(rt_mock_platform: Runtime) -> None:
    from platynui_native import Point, PointerOverrides, Rect

    _assert_frompyobject_ok(
        lambda: rt_mock_platform.pointer_move_to(Point(0.0, 0.0), overrides=PointerOverrides(origin='desktop'))
    )
    _assert_frompyobject_ok(
        lambda: rt_mock_platform.pointer_move_to(Point(0.0, 0.0), overrides=PointerOverrides(origin=Point(1.0, 2.0)))
    )
    _assert_frompyobject_ok(
        lambda: rt_mock_platform.pointer_move_to(
            Point(0.0, 0.0), overrides=PointerOverrides(origin=Rect(1.0, 2.0, 3.0, 4.0))
        )
    )


@pytest.mark.parametrize('button', [1, 2, 3, 5])
def test_pointer_button_accepts_enum_and_int(button: int, rt_mock_platform: Runtime) -> None:
    from platynui_native import Point

    _assert_frompyobject_ok(lambda: rt_mock_platform.pointer_click(Point(0.0, 0.0), button=button))


def test_pointer_button_enum_is_accepted(rt_mock_platform: Runtime) -> None:
    from platynui_native import Point, PointerButton

    _assert_frompyobject_ok(lambda: rt_mock_platform.pointer_click(Point(0.0, 0.0), button=PointerButton.LEFT))
    _assert_frompyobject_ok(lambda: rt_mock_platform.pointer_click(Point(0.0, 0.0), button=PointerButton.MIDDLE))
    _assert_frompyobject_ok(lambda: rt_mock_platform.pointer_click(Point(0.0, 0.0), button=PointerButton.RIGHT))


def test_keyboard_overrides_class_is_required(rt_mock_platform: Runtime) -> None:
    from platynui_native import KeyboardOverrides

    kov = KeyboardOverrides(between_keys_delay_ms=2)
    _assert_frompyobject_ok(lambda: rt_mock_platform.keyboard_type('a', overrides=kov))
