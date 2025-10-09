import pytest
from platynui_native import Runtime, Rect, ProviderError


def test_screenshot_png_bytes():
    rt = Runtime()
    try:
        data = rt.screenshot(Rect(0, 0, 10, 10), "image/png")
    except ProviderError:
        pytest.skip("Screenshot provider not available in this build")
    assert isinstance(data, (bytes, bytearray))
    assert data.startswith(b"\x89PNG\r\n\x1a\n")

    # default mime
    data2 = rt.screenshot(Rect(0, 0, 5, 5))
    assert isinstance(data2, (bytes, bytearray))


def test_highlight_rects_smoke():
    rt = Runtime()
    try:
        rt.highlight([Rect(0, 0, 5, 5)], duration_ms=10)
        rt.clear_highlight()
    except ProviderError:
        pytest.skip("Highlight provider not available in this build")
