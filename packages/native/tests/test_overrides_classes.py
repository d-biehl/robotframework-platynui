from collections.abc import Callable
from typing import Any

import pytest
from platynui_native import Runtime


def _assert_ok(call: Callable[[], Any]) -> None:
    try:
        call()
    except Exception as e:
        if type(e).__name__ not in {'PointerError', 'KeyboardError'}:
            raise


def test_pointer_overrides_class_is_accepted(rt_mock_platform: Runtime) -> None:
    from platynui_native import Point, PointerOverrides

    ov = PointerOverrides(
        speed_factor=1.2,
        origin=Point(0.0, 0.0),
        after_move_delay_ms=15,
        scroll_step=(1.0, -2.0),
    )
    _assert_ok(lambda: rt_mock_platform.pointer_move_to(Point(0.0, 0.0), overrides=ov))
    _assert_ok(lambda: rt_mock_platform.pointer_click(Point(0.0, 0.0), overrides=ov))

    # getters
    assert ov.speed_factor == 1.2
    assert isinstance(ov.origin, Point)
    assert ov.after_move_delay_ms == 15
    assert ov.scroll_step == (1.0, -2.0)


def test_keyboard_overrides_class_is_accepted(rt_mock_platform: Runtime) -> None:
    from platynui_native import KeyboardOverrides

    kov = KeyboardOverrides(between_keys_delay_ms=2, press_delay_ms=3)
    _assert_ok(lambda: rt_mock_platform.keyboard_type('abc', overrides=kov))
    # getters
    assert kov.between_keys_delay_ms == 2
    assert kov.press_delay_ms == 3


def test_origin_accepts_core_point_and_rect_and_returns_objects(rt_mock_platform: Runtime) -> None:
    from platynui_native import Point, PointerOverrides, Rect

    # Absolute via Point
    ov1 = PointerOverrides(origin=Point(10.0, 20.0))
    assert isinstance(ov1.origin, Point)
    _assert_ok(lambda: rt_mock_platform.pointer_move_to(Point(0.0, 0.0), overrides=ov1))

    # Bounds via Rect
    ov2 = PointerOverrides(origin=Rect(50.0, 60.0, 200.0, 100.0))
    assert isinstance(ov2.origin, Rect)
    _assert_ok(lambda: rt_mock_platform.pointer_move_to(Point(0.0, 0.0), overrides=ov2))

    # Dict forms removed: only 'desktop' | Point | Rect are accepted


# -------- Paced approach (spec: pointer-motion) --------


def test_the_default_profile_paces_three_steps_20_ms_apart(rt_mock_platform: Runtime) -> None:
    profile = rt_mock_platform.pointer_profile()
    assert profile.approach_steps == 3
    assert profile.approach_interval_ms == 20


def test_the_approach_round_trips_through_the_classes(rt_mock_platform: Runtime) -> None:
    from platynui_native import Point, PointerOverrides, PointerProfile

    ov = PointerOverrides(approach_steps=5, approach_interval_ms=30)
    assert ov.approach_steps == 5
    assert ov.approach_interval_ms == 30
    _assert_ok(lambda: rt_mock_platform.pointer_move_to(Point(10.0, 10.0), overrides=ov))

    profile = PointerProfile(approach_steps=2, approach_interval_ms=12)
    assert profile.approach_steps == 2
    assert profile.approach_interval_ms == 12
    rt_mock_platform.set_pointer_profile(profile)
    current = rt_mock_platform.pointer_profile()
    assert current.approach_steps == 2
    assert current.approach_interval_ms == 12


def test_the_approach_round_trips_through_the_dicts(rt_mock_platform: Runtime) -> None:
    from platynui_native import Point

    rt_mock_platform.set_pointer_profile({'approach_steps': 4, 'approach_interval_ms': 25})
    profile = rt_mock_platform.pointer_profile()
    assert profile.approach_steps == 4
    assert profile.approach_interval_ms == 25
    rt_mock_platform.pointer_move_to(Point(10.0, 10.0), overrides={'approach_steps': 0})
    assert rt_mock_platform.pointer_position() == Point(10.0, 10.0)


@pytest.mark.parametrize('steps', [-1, 2.5])
def test_an_invalid_approach_step_count_names_the_key_and_does_not_move(
    steps: object, rt_mock_platform: Runtime
) -> None:
    from platynui_native import Point

    rt_mock_platform.pointer_move_to(Point(10.0, 10.0))
    bad: Any = {'approach_steps': steps}
    with pytest.raises((TypeError, OverflowError), match=r'approach_steps'):
        rt_mock_platform.pointer_move_to(Point(40.0, 40.0), overrides=bad)
    assert rt_mock_platform.pointer_position() == Point(10.0, 10.0)


def test_a_negative_approach_interval_counts_as_zero() -> None:
    from platynui_native import PointerOverrides, PointerProfile

    assert PointerOverrides(approach_interval_ms=-5).approach_interval_ms == 0
    assert PointerProfile(approach_interval_ms=-5).approach_interval_ms == 0
