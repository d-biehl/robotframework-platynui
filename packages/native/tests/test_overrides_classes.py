def _assert_ok(call):
    try:
        call()
    except Exception as e:  # noqa: BLE001
        if type(e).__name__ not in {"PointerError", "KeyboardError"}:
            raise


def test_pointer_overrides_class_is_accepted():
    from platynui_native import Runtime, Point, PointerOverrides

    rt = Runtime()
    ov = PointerOverrides(
        speed_factor=1.2,
        origin=Point(0.0, 0.0),
        after_move_delay_ms=15,
        scroll_step=(1.0, -2.0),
    )
    _assert_ok(lambda: rt.pointer_move_to(Point(0.0, 0.0), overrides=ov))
    _assert_ok(lambda: rt.pointer_click(Point(0.0, 0.0), overrides=ov))

    # getters
    assert ov.speed_factor == 1.2
    assert isinstance(ov.origin, Point)
    assert ov.after_move_delay_ms == 15
    assert ov.scroll_step == (1.0, -2.0)


def test_keyboard_overrides_class_is_accepted():
    from platynui_native import Runtime, KeyboardOverrides

    rt = Runtime()
    kov = KeyboardOverrides(between_keys_delay_ms=2, press_delay_ms=3)
    _assert_ok(lambda: rt.keyboard_type("abc", overrides=kov))
    # getters
    assert kov.between_keys_delay_ms == 2
    assert kov.press_delay_ms == 3


def test_origin_accepts_core_point_and_rect_and_returns_objects():
    from platynui_native import Runtime, Point, Rect, PointerOverrides

    rt = Runtime()

    # Absolute via Point
    ov1 = PointerOverrides(origin=Point(10.0, 20.0))
    assert isinstance(ov1.origin, Point)
    _assert_ok(lambda: rt.pointer_move_to(Point(0.0, 0.0), overrides=ov1))

    # Bounds via Rect
    ov2 = PointerOverrides(origin=Rect(50.0, 60.0, 200.0, 100.0))
    assert isinstance(ov2.origin, Rect)
    _assert_ok(lambda: rt.pointer_move_to(Point(0.0, 0.0), overrides=ov2))

    # Dict forms removed: only 'desktop' | Point | Rect are accepted
