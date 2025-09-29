def _assert_ok(call):
    try:
        call()
    except Exception as e:  # noqa: BLE001
        if type(e).__name__ not in {"PointerError", "KeyboardError"}:
            raise


def test_pointer_overrides_class_is_accepted():
    from platynui_native import runtime, core

    rt = runtime.Runtime()
    ov = runtime.PointerOverrides(speed_factor=1.2, origin=core.Point(0.0, 0.0),
                                  after_move_delay_ms=15, scroll_step=(1.0, -2.0))
    _assert_ok(lambda: rt.pointer_move_to(core.Point(0.0, 0.0), overrides=ov))
    _assert_ok(lambda: rt.pointer_click(core.Point(0.0, 0.0), overrides=ov))

    # getters
    assert ov.speed_factor == 1.2
    assert isinstance(ov.origin, core.Point)
    assert ov.after_move_delay_ms == 15
    assert ov.scroll_step == (1.0, -2.0)


def test_keyboard_overrides_class_is_accepted():
    from platynui_native import runtime

    rt = runtime.Runtime()
    kov = runtime.KeyboardOverrides(between_keys_delay_ms=2, press_delay_ms=3)
    _assert_ok(lambda: rt.keyboard_type("abc", overrides=kov))
    # getters
    assert kov.between_keys_delay_ms == 2
    assert kov.press_delay_ms == 3


def test_origin_accepts_core_point_and_rect_and_returns_objects():
    from platynui_native import runtime, core

    rt = runtime.Runtime()

    # Absolute via core.Point
    ov1 = runtime.PointerOverrides(origin=core.Point(10.0, 20.0))
    assert isinstance(ov1.origin, core.Point)
    _assert_ok(lambda: rt.pointer_move_to(core.Point(0.0, 0.0), overrides=ov1))

    # Bounds via core.Rect
    ov2 = runtime.PointerOverrides(origin=core.Rect(50.0, 60.0, 200.0, 100.0))
    assert isinstance(ov2.origin, core.Rect)
    _assert_ok(lambda: rt.pointer_move_to(core.Point(0.0, 0.0), overrides=ov2))

    # Dict forms removed: only 'desktop' | core.Point | core.Rect are accepted
