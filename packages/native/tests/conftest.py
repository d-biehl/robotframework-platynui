import pytest

import platynui_native as pn


from typing import Generator


@pytest.fixture(scope="function")
def rt_mock_platform() -> Generator[pn.Runtime, None, None]:
    """Runtime with mock UI provider and mock platform devices.

    Includes pointer, keyboard, highlight, screenshot and (if available)
    mock desktop info devices for deterministic behavior.
    """
    platforms = pn.PlatformOverrides()
    # # Desktop info via mock platform
    # platforms.desktop_info = pn.MOCK_PLATFORM
    # platforms.highlight = pn.MOCK_HIGHLIGHT_PROVIDER
    # platforms.screenshot = pn.MOCK_SCREENSHOT_PROVIDER
    # platforms.pointer = pn.MOCK_POINTER_DEVICE
    # platforms.keyboard = pn.MOCK_KEYBOARD_DEVICE

    runtime = pn.Runtime.new_with_providers_and_platforms([pn.MOCK_PROVIDER], platforms)
    try:
        yield runtime
    finally:
        runtime.shutdown()
