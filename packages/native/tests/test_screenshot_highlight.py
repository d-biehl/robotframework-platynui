from platynui_native import Rect, Runtime


def test_screenshot_png_bytes(rt_mock_platform: Runtime) -> None:
    data = rt_mock_platform.screenshot(Rect(0, 0, 10, 10), 'image/png')
    assert isinstance(data, (bytes, bytearray))
    assert data.startswith(b'\x89PNG\r\n\x1a\n')

    # default mime
    data2 = rt_mock_platform.screenshot(Rect(0, 0, 5, 5))
    assert isinstance(data2, (bytes, bytearray))


def test_highlight_rects_smoke(rt_mock_platform: Runtime) -> None:
    rt_mock_platform.highlight([Rect(0, 0, 5, 5)], duration_ms=10)
    rt_mock_platform.clear_highlight()
