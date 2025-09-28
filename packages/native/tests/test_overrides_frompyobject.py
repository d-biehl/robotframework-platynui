import pytest


def _assert_frompyobject_ok(call):
    try:
        call()
    except Exception as e:  # noqa: BLE001
        # Accept device errors; conversion succeeded if class name matches
        if type(e).__name__ not in {"PointerError", "KeyboardError"}:
            raise


def test_pointer_overrides_accept_multiple_origin_shapes():
    from platynui_native import runtime

    rt = runtime.Runtime()

    forms = [
        {"origin": "desktop"},
        {"origin": (10.0, 20.0)},
        {"origin": (10.0, 20.0, 100.0, 50.0)},
        {"origin": {"absolute": (10.0, 20.0)}},
        {"origin": {"bounds": (10.0, 20.0, 100.0, 50.0)}},
    ]

    for ov in forms:
        _assert_frompyobject_ok(lambda: rt.pointer_move_to((0.0, 0.0), overrides=ov))


@pytest.mark.parametrize("button", ["left", "middle", "right", 5])
def test_pointer_button_accepts_str_and_int(button):
    from platynui_native import runtime

    rt = runtime.Runtime()
    _assert_frompyobject_ok(lambda: rt.pointer_click((0.0, 0.0), button=button))


def test_keyboard_overrides_dict_is_parsed():
    from platynui_native import runtime

    rt = runtime.Runtime()
    ov = {
        "press_delay_ms": 5,
        "release_delay_ms": 5,
        "between_keys_delay_ms": 1,
        "chord_press_delay_ms": 1,
        "chord_release_delay_ms": 1,
        "after_sequence_delay_ms": 2,
        "after_text_delay_ms": 2,
    }
    _assert_frompyobject_ok(lambda: rt.keyboard_type("a", overrides=ov))
