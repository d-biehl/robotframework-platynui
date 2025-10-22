import pytest

import platynui_native as pn


from typing import Generator


@pytest.fixture(scope="function")
def rt_mock_platform() -> Generator[pn.Runtime, None, None]:
    """Runtime with mock UI provider and mock platform devices.

    Includes pointer, keyboard, highlight, screenshot and (if available)
    mock desktop info devices for deterministic behavior.
    """
    runtime = pn.Runtime.new_with_mock()
    try:
        yield runtime
    finally:
        runtime.shutdown()
