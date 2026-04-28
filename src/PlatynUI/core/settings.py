# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Process-wide settings singleton.

`Settings` holds timing and behaviour knobs for the wait/ensure
helpers and the device proxies. Instances are frozen, slotted and
keyword-only and act as context managers that temporarily replace the
process-wide singleton::

    with Settings(ensure_timeout=30):
        ensure_that(ctx, predicate)

For programmatic configuration without nesting, use
`set_current`. The accessor is **not** thread-safe; this
matches typical ``pabot`` parallelism where each worker runs in its own
process.
"""

from dataclasses import dataclass
from typing import Literal

__all__ = ['Settings']


_current: 'Settings | None' = None
_stack: list['Settings'] = []


@dataclass(frozen=True, slots=True, kw_only=True)
class Settings:
    """Tunable timings and limits used across the PlatynUI core.

    All values are seconds unless noted otherwise. Construct with
    keyword arguments to override individual fields; unspecified fields
    keep their defaults.
    """

    # Wait timings
    wait_for_timeout: float = 1.0
    wait_for_delay: float = 0.1
    ensure_timeout: float = 15.0
    ensure_delay: float = 0.1
    exists_timeout: float = 1.0
    window_close_timeout: float = 1.0
    application_exit_timeout: float = 10.0

    # Pointer delay overrides (milliseconds; ``None`` keeps the active profile value).
    # Mirror the timing fields of ``platynui_native.PointerOverridesDict``.
    pointer_after_move_delay_ms: float | None = None
    pointer_after_input_delay_ms: float | None = None
    pointer_press_release_delay_ms: float | None = None
    pointer_after_click_delay_ms: float | None = None
    pointer_before_next_click_delay_ms: float | None = None
    pointer_multi_click_delay_ms: float | None = None

    # Keyboard delay overrides (milliseconds; ``None`` keeps the active profile value).
    # Mirror ``platynui_native.KeyboardOverridesDict``.
    keyboard_press_delay_ms: float | None = None
    keyboard_release_delay_ms: float | None = None
    keyboard_between_keys_delay_ms: float | None = None
    keyboard_chord_press_delay_ms: float | None = None
    keyboard_chord_release_delay_ms: float | None = None
    keyboard_after_sequence_delay_ms: float | None = None
    keyboard_after_text_delay_ms: float | None = None

    # Display / diagnostics
    display_screenshot_format: str = 'png'
    display_screenshot_quality: int = -1
    display_screenshot_basename: str = 'screenshot'
    element_highlight_time: float = 2.0
    element_highlight_ensure_timeout: float = 2.0

    @staticmethod
    def current() -> 'Settings':
        """Return the active settings, creating defaults on first use."""
        global _current
        if _current is None:
            _current = Settings()
        return _current

    @staticmethod
    def set_current(settings: 'Settings') -> None:
        """Replace the process-wide singleton.

        Prefer the context-manager form inside a ``with`` block so the
        previous value is restored automatically.
        """
        global _current
        _current = settings

    def __enter__(self) -> 'Settings':
        global _current
        _stack.append(_current if _current is not None else Settings())
        _current = self
        return self

    def __exit__(self, exc_type: object, exc_val: object, exc_tb: object) -> Literal[False]:
        global _current
        _current = _stack.pop()
        return False
