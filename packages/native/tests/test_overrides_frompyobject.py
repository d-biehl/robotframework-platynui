import pytest


def _assert_frompyobject_ok(call):
    try:
        call()
    except Exception as e:  # noqa: BLE001
        # Accept device errors; conversion succeeded if class name matches
        if type(e).__name__ not in {"PointerError", "KeyboardError"}:
            raise


def test_pointer_overrides_accept_origin_desktop_point_rect():
    from platynui_native import runtime, core

    rt = runtime.Runtime()
    _assert_frompyobject_ok(lambda: rt.pointer_move_to(core.Point(0.0, 0.0), overrides=runtime.PointerOverrides(origin="desktop")))
    _assert_frompyobject_ok(lambda: rt.pointer_move_to(core.Point(0.0, 0.0), overrides=runtime.PointerOverrides(origin=core.Point(1.0, 2.0))))
    _assert_frompyobject_ok(lambda: rt.pointer_move_to(core.Point(0.0, 0.0), overrides=runtime.PointerOverrides(origin=core.Rect(1.0, 2.0, 3.0, 4.0))))


@pytest.mark.parametrize("button", [1, 2, 3, 5])
def test_pointer_button_accepts_enum_and_int(button):
    from platynui_native import runtime, core

    rt = runtime.Runtime()
    _assert_frompyobject_ok(lambda: rt.pointer_click(core.Point(0.0, 0.0), button=button))


def test_pointer_button_enum_is_accepted():
    from platynui_native import runtime, core

    rt = runtime.Runtime()
    _assert_frompyobject_ok(lambda: rt.pointer_click(core.Point(0.0, 0.0), button=runtime.PointerButton.LEFT))
    _assert_frompyobject_ok(lambda: rt.pointer_click(core.Point(0.0, 0.0), button=runtime.PointerButton.MIDDLE))
    _assert_frompyobject_ok(lambda: rt.pointer_click(core.Point(0.0, 0.0), button=runtime.PointerButton.RIGHT))


def test_keyboard_overrides_class_is_required():
    from platynui_native import runtime

    rt = runtime.Runtime()
    kov = runtime.KeyboardOverrides(between_keys_delay_ms=2)
    _assert_frompyobject_ok(lambda: rt.keyboard_type("a", overrides=kov))
